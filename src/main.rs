use sycamore::prelude::*;


fn main() {
    sycamore::render(|cx| view!{ cx,
        p (class="test") { "Inicio do template" }
    })
}
