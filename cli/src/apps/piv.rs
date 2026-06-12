app!();

impl<'t> crate::Select<'t> for App<'t> {
    const RID: &'static [u8] = super::Rid::NIST;
    const PIX: &'static [u8] = super::Pix::PIV;
}
