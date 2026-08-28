//! L2 — AudioAdapter. Sistem ses seviyesi ve mute.
//!
//! Tetik `IAudioEndpointVolumeCallback::OnNotify`: donanım tuşu, mixer, kulaklık
//! — komutun nereden geldiği önemli değil, uygulamanın tüm fikri bu.
//! Varsayılan cihaz değişimi `IMMNotificationClient` ile yakalanıyor.
//!
//! Kendi thread'i, kendi apartment'ı. Adapter durum tutmaz; okuduğunu domain
//! tipine çevirip mpsc'ye yazar.

use std::sync::mpsc::{sync_channel, Sender, SyncSender};
use std::sync::Mutex;

use windows::core::{implement, Result as WinResult, PCWSTR};
use windows::Win32::Devices::FunctionDiscovery::PKEY_Device_FriendlyName;
use windows::Win32::Foundation::PROPERTYKEY;
use windows::Win32::Media::Audio::Endpoints::{
    IAudioEndpointVolume, IAudioEndpointVolumeCallback, IAudioEndpointVolumeCallback_Impl,
};
use windows::Win32::Media::Audio::{
    eMultimedia, eRender, EDataFlow, ERole, IMMDevice, IMMDeviceEnumerator, IMMNotificationClient,
    IMMNotificationClient_Impl, MMDeviceEnumerator, AUDIO_VOLUME_NOTIFICATION_DATA, DEVICE_STATE,
};
use windows::Win32::System::Com::StructuredStorage::PropVariantClear;
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CLSCTX_ALL, COINIT_MULTITHREADED, STGM_READ,
};
use windows::Win32::System::Variant::VT_LPWSTR;

use crate::core::model::VolumeSnapshot;
use crate::core::DomainEvent;

pub fn spawn(tx: Sender<DomainEvent>) -> std::io::Result<()> {
    std::thread::Builder::new().name("audio".into()).spawn(move || {
        unsafe {
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
        }
        if let Err(e) = run(tx) {
            eprintln!("[audio] baslatilamadi: {e} (HRESULT {:#010x})", e.code().0);
        }
    })?;
    Ok(())
}

enum Cmd {
    /// Varsayılan çıkış cihazı değişti; aboneliği yeni cihaza taşı.
    Rebind,
}

/// Bir uç noktaya bağlı abonelik. `cb` düşerse bildirim gelmez — bu yüzden
/// kayıt süresince tutuluyor.
struct Bound {
    volume: IAudioEndpointVolume,
    cb: IAudioEndpointVolumeCallback,
}

impl Bound {
    fn detach(&self) {
        unsafe {
            let _ = self.volume.UnregisterControlChangeNotify(&self.cb);
        }
    }
}

fn run(tx: Sender<DomainEvent>) -> WinResult<()> {
    let enumerator: IMMDeviceEnumerator =
        unsafe { CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)? };

    let (cmd_tx, cmd_rx) = sync_channel::<Cmd>(4);
    let watcher: IMMNotificationClient = DeviceWatcher { tx: Mutex::new(cmd_tx) }.into();
    unsafe { enumerator.RegisterEndpointNotificationCallback(&watcher)? };

    let mut bound = bind(&enumerator, &tx);

    // Bildirimler sistem thread'lerinde geliyor; bu thread yalnızca cihaz
    // değişiminde yeniden bağlamak için bekliyor.
    while let Ok(Cmd::Rebind) = cmd_rx.recv() {
        if let Some(b) = bound.take() {
            b.detach();
        }
        bound = bind(&enumerator, &tx);
    }

    if let Some(b) = bound.take() {
        b.detach();
    }
    unsafe {
        let _ = enumerator.UnregisterEndpointNotificationCallback(&watcher);
    }
    Ok(())
}

/// Varsayılan çıkış cihazına bağlanır ve bağlanma anındaki seviyeyi bir olay
/// olarak gönderir. Açılışta bu olayı 1. gürültü kuralı yutar; cihaz değişiminde
/// ise gerçekten yeni bir bilgi taşır.
fn bind(enumerator: &IMMDeviceEnumerator, tx: &Sender<DomainEvent>) -> Option<Bound> {
    let device = unsafe { enumerator.GetDefaultAudioEndpoint(eRender, eMultimedia) }.ok()?;
    let device_name = friendly_name(&device).unwrap_or_default();

    let volume: IAudioEndpointVolume = unsafe { device.Activate(CLSCTX_ALL, None) }.ok()?;
    let cb: IAudioEndpointVolumeCallback = VolumeCallback {
        tx: Mutex::new(tx.clone()),
        device_name: device_name.clone(),
    }
    .into();
    unsafe { volume.RegisterControlChangeNotify(&cb) }.ok()?;

    let level = unsafe { volume.GetMasterVolumeLevelScalar() }.unwrap_or(0.0);
    let muted = unsafe { volume.GetMute() }.map(|b| b.as_bool()).unwrap_or(false);
    let _ = tx.send(DomainEvent::Volume(VolumeSnapshot { level, muted, device_name }));

    Some(Bound { volume, cb })
}

/// `docs/DESIGN.md` ses kutusundaki `.fly__source` — "Realtek · Hoparlör".
fn friendly_name(device: &IMMDevice) -> Option<String> {
    unsafe {
        let store = device.OpenPropertyStore(STGM_READ).ok()?;
        let mut pv = store.GetValue(&PKEY_Device_FriendlyName).ok()?;
        let inner = &pv.Anonymous.Anonymous;
        let name = if inner.vt == VT_LPWSTR {
            inner.Anonymous.pwszVal.to_string().ok()
        } else {
            None
        };
        let _ = PropVariantClear(&mut pv);
        name
    }
}

#[implement(IAudioEndpointVolumeCallback)]
struct VolumeCallback {
    tx: Mutex<Sender<DomainEvent>>,
    device_name: String,
}

impl IAudioEndpointVolumeCallback_Impl for VolumeCallback_Impl {
    fn OnNotify(&self, pnotify: *mut AUDIO_VOLUME_NOTIFICATION_DATA) -> WinResult<()> {
        let Some(data) = (unsafe { pnotify.as_ref() }) else {
            return Ok(());
        };
        let ev = DomainEvent::Volume(VolumeSnapshot {
            level: data.fMasterVolume,
            muted: data.bMuted.as_bool(),
            device_name: self.device_name.clone(),
        });
        if let Ok(tx) = self.tx.lock() {
            let _ = tx.send(ev);
        }
        Ok(())
    }
}

#[implement(IMMNotificationClient)]
struct DeviceWatcher {
    tx: Mutex<SyncSender<Cmd>>,
}

impl IMMNotificationClient_Impl for DeviceWatcher_Impl {
    fn OnDefaultDeviceChanged(
        &self,
        flow: EDataFlow,
        role: ERole,
        _id: &PCWSTR,
    ) -> WinResult<()> {
        // Yalnızca kullanıcının duyduğu çıkış yolu ilgilendiriyor.
        if flow == eRender && role == eMultimedia {
            if let Ok(tx) = self.tx.lock() {
                let _ = tx.try_send(Cmd::Rebind);
            }
        }
        Ok(())
    }

    fn OnDeviceStateChanged(&self, _id: &PCWSTR, _state: DEVICE_STATE) -> WinResult<()> {
        Ok(())
    }
    fn OnDeviceAdded(&self, _id: &PCWSTR) -> WinResult<()> {
        Ok(())
    }
    fn OnDeviceRemoved(&self, _id: &PCWSTR) -> WinResult<()> {
        Ok(())
    }
    fn OnPropertyValueChanged(&self, _id: &PCWSTR, _key: &PROPERTYKEY) -> WinResult<()> {
        Ok(())
    }
}
