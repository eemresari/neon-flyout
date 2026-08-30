//! Kutunun ekrandaki konumu — **saf aritmetik**. Windows tipi yok; `MONITORINFO`
//! okumak `adapters/window.rs`'in işi, hesap burada testlenebilir duruyor.

use super::config::Corner;

/// Monitörün görev çubuğu düşülmüş çalışma alanı, fiziksel piksel.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WorkArea {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

/// Pencerenin sol-üst köşesi nereye konacak.
///
/// Kutu çalışma alanından büyükse boşluk uygulanmaz ve sol/üst kenara yapışır —
/// ekran dışına taşıp tamamen görünmez olmasındansa kırpılması yeğdir.
pub fn position(work: WorkArea, w: i32, h: i32, corner: Corner, margin: i32) -> (i32, i32) {
    let sag = (work.right - w - margin).max(work.left);
    let alt = (work.bottom - h - margin).max(work.top);
    let sol = (work.left + margin).min(sag);
    let ust = (work.top + margin).min(alt);

    match corner {
        Corner::TopRight => (sag, ust),
        Corner::TopLeft => (sol, ust),
        Corner::BottomRight => (sag, alt),
        Corner::BottomLeft => (sol, alt),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::config::Corner;

    /// 1920×1080 ekran, görev çubuğu altta 40 px: çalışma alanı 0,0–1920,1040.
    const WORK: WorkArea = WorkArea {
        left: 0,
        top: 0,
        right: 1920,
        bottom: 1040,
    };

    #[test]
    fn sag_ust() {
        assert_eq!(position(WORK, 400, 120, Corner::TopRight, 16), (1504, 16));
    }

    #[test]
    fn sol_ust() {
        assert_eq!(position(WORK, 400, 120, Corner::TopLeft, 16), (16, 16));
    }

    #[test]
    fn sag_alt() {
        assert_eq!(position(WORK, 400, 120, Corner::BottomRight, 16), (1504, 904));
    }

    #[test]
    fn sol_alt() {
        assert_eq!(position(WORK, 400, 120, Corner::BottomLeft, 16), (16, 904));
    }

    #[test]
    fn ikincil_monitorun_negatif_koordinatlari_korunur() {
        // Soldaki ikinci ekran: koordinatlar negatif. Boşluk yine kenardan.
        let work = WorkArea {
            left: -1920,
            top: 0,
            right: 0,
            bottom: 1040,
        };
        assert_eq!(position(work, 400, 120, Corner::TopRight, 16), (-416, 16));
    }

    #[test]
    fn kutu_calisma_alanindan_genisse_tasmaz() {
        // Patolojik durum: pencere ekrandan geniş. Sol/üst kenara yapış.
        let work = WorkArea {
            left: 0,
            top: 0,
            right: 300,
            bottom: 100,
        };
        assert_eq!(position(work, 400, 120, Corner::TopRight, 16), (0, 0));
    }
}
