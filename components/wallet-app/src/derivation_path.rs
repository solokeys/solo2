use iso7816::Status;

/// BIP derivation path component
pub type PathComponent = u32;

/// Parsed BIP derivation path
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DerivationPath<'a> {
    /// Number of path components (2, 3, or 4)
    pub depth: u8,
    /// Path components (each is 4 bytes, big-endian u32)
    pub components: &'a [u8],
}

impl<'a> DerivationPath<'a> {
    /// Parse a serialized derivation path
    ///
    /// Format:
    /// - First byte: depth (number of components, typically 2-4)
    /// - Followed by depth * 4 bytes: each component as u32 big-endian
    pub fn parse(data: &'a [u8]) -> Result<Self, Status> {
        if data.is_empty() {
            return Err(Status::WrongLength);
        }

        let depth = data[0];
        if !(2..=10).contains(&depth) {
            return Err(Status::IncorrectDataParameter);
        }

        let expected_len = 1 + (depth as usize * 4);
        if data.len() < expected_len {
            return Err(Status::WrongLength);
        }

        let components = &data[1..expected_len];

        Ok(DerivationPath { depth, components })
    }

    /// Get a specific path component by index
    pub fn component(&self, index: usize) -> Option<PathComponent> {
        if index >= self.depth as usize {
            return None;
        }

        let offset = index * 4;
        if offset + 4 > self.components.len() {
            return None;
        }

        let bytes = [
            self.components[offset],
            self.components[offset + 1],
            self.components[offset + 2],
            self.components[offset + 3],
        ];
        Some(u32::from_be_bytes(bytes))
    }

    /// Get all path components as a vector
    ///
    /// Note: This returns a Vec instead of an Iterator to avoid lifetime issues.
    /// For iterator-based access, use component() in a loop.
    pub fn components_vec(&self) -> heapless::Vec<PathComponent, 10> {
        let mut vec = heapless::Vec::new();
        for i in 0..self.depth as usize {
            if let Some(comp) = self.component(i) {
                vec.push(comp).ok();
            }
        }
        vec
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_derivation_path() {
        // Test path: m/44'/501'/0'
        // Serialized: [0x03, 0x80, 0x00, 0x00, 0x2c, 0x80, 0x01, 0xf5, 0x80, 0x00, 0x00, 0x00, 0x00]
        let data = [
            0x03, // depth = 3
            0x80, 0x00, 0x00, 0x2c, // 44' (0x8000002c)
            0x80, 0x01, 0xf5, 0x00, // 501' (0x8001f500)
            0x80, 0x00, 0x00, 0x00, // 0' (0x80000000)
        ];

        let path = DerivationPath::parse(&data).unwrap();
        assert_eq!(path.depth, 3);
        assert_eq!(path.component(0), Some(0x8000002c));
        assert_eq!(path.component(1), Some(0x8001f500));
        assert_eq!(path.component(2), Some(0x80000000));
    }

    #[test]
    fn test_parse_invalid_path() {
        // Empty data
        assert!(DerivationPath::parse(&[]).is_err());

        // Depth too small
        assert!(DerivationPath::parse(&[0x01]).is_err());

        // Depth too large
        assert!(DerivationPath::parse(&[0x0b]).is_err());

        // Insufficient data
        assert!(DerivationPath::parse(&[0x03, 0x00, 0x00]).is_err());
    }
}
