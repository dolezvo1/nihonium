
trait NHView {
    
    // TODO: modular layers system to allow hiding
    fn is_hidden(&self) -> bool;
    fn draw_in(&self, impl NHCanvas);
    
}