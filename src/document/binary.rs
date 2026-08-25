#[derive(Debug, Default)]
pub struct BinaryBuffer {
    pub bytes: Vec<u8>,
}

pub const DEFAULT_BINARY_SCAN_LIMIT: usize = 8 * 1024;
const CONTROL_RATIO_THRESHOLD: f32 = 0.30;
const MIN_CONTROL_BYTES: usize = 3;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BinaryHeuristic {
    pub scanned_len: usize,
    pub nul_bytes: usize,
    pub suspicious_control_bytes: usize,
}

impl BinaryHeuristic {
    pub fn is_binary_conservative(&self) -> bool {
        if self.nul_bytes > 0 {
            return true;
        }

        if self.scanned_len == 0 {
            return false;
        }

        let ratio = self.suspicious_control_bytes as f32 / self.scanned_len as f32;
        self.suspicious_control_bytes >= MIN_CONTROL_BYTES && ratio > CONTROL_RATIO_THRESHOLD
    }
}

pub fn inspect_binary(bytes: &[u8], scan_limit: usize) -> BinaryHeuristic {
    let scan_len = bytes.len().min(scan_limit.max(1));
    let mut nul_bytes = 0;
    let mut suspicious_control_bytes = 0;

    for byte in &bytes[..scan_len] {
        if *byte == 0 {
            nul_bytes += 1;
            continue;
        }

        let is_suspicious_control = (*byte < 0x20
            && !matches!(*byte, b'\n' | b'\r' | b'\t' | 0x08 | 0x0c))
            || *byte == 0x7f;
        if is_suspicious_control {
            suspicious_control_bytes += 1;
        }
    }

    BinaryHeuristic {
        scanned_len: scan_len,
        nul_bytes,
        suspicious_control_bytes,
    }
}
