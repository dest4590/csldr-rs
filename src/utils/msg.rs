pub struct MsgRead<'a> {
    data: &'a [u8],
    ofs: usize,
}

impl<'a> MsgRead<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Self { data, ofs: 0 }
    }

    pub fn ofs(&self) -> usize {
        self.ofs
    }

    pub fn set_ofs(&mut self, ofs: usize) {
        self.ofs = ofs;
    }

    pub fn seek(&mut self, size: usize) -> bool {
        if self.ofs + size > self.data.len() {
            false
        } else {
            self.ofs += size;
            true
        }
    }

    pub fn read_data(&mut self, buffer: &mut [u8]) -> bool {
        let size = buffer.len();
        if self.ofs + size > self.data.len() {
            false
        } else {
            buffer.copy_from_slice(&self.data[self.ofs..self.ofs + size]);
            self.ofs += size;
            true
        }
    }

    pub fn read_byte(&mut self) -> Option<u8> {
        let b = *self.data.get(self.ofs)?;
        self.ofs += 1;
        Some(b)
    }

    pub fn read_char(&mut self) -> Option<i8> {
        self.read_byte().map(|b| b as i8)
    }

    pub fn read_short(&mut self) -> Option<i16> {
        let bytes: [u8; 2] = self.data.get(self.ofs..self.ofs + 2)?.try_into().ok()?;
        self.ofs += 2;
        Some(i16::from_le_bytes(bytes))
    }

    pub fn read_angle(&mut self) -> Option<f32> {
        self.read_char().map(|c| (c as f32) * (360.0 / 256.0))
    }

    pub fn read_coord(&mut self) -> Option<f32> {
        self.read_short().map(|s| (s as f32) * (1.0 / 8.0))
    }

    pub fn read_string(&mut self) -> String {
        let mut result = Vec::new();
        while let Some(ch) = self.read_byte() {
            if ch == 0 {
                break;
            }
            result.push(ch);
        }
        String::from_utf8_lossy(&result).into_owned()
    }
}
