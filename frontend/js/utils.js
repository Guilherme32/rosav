
function getPromiseFromEvent(item, event) {
  return new Promise((resolve) => {
    const listener = () => {
      item.removeEventListener(event, listener);
      resolve();
    }
    item.addEventListener(event, listener);
  })
}

function getPromiseFromLeftButtonEvent(item, event) {
  return new Promise((resolve) => {
    const listener = (e) => {
      if(e.button === 0) {
        item.removeEventListener(event, listener);
        resolve();
      }
    }
    item.addEventListener(event, listener);
  })
}

function getPromiseFromRightButtonEvent() {
  return new Promise((resolve) => {
    const listener = (e) => {
      document.removeEventListener("contextmenu", listener);
      resolve();
    }
    document.addEventListener("contextmenu", listener);
  })
}

export async function wait_for_pointer_down() {
  var response = await getPromiseFromLeftButtonEvent(document, "mousedown");
}

export async function wait_for_pointer_up() {
  await getPromiseFromLeftButtonEvent(document, "mouseup")
}

export async function wait_for_right_button_down() {
  await getPromiseFromRightButtonEvent()
}

addEventListener("mousemove", (event) => {
  pointer_x = event.clientX;
  pointer_y = event.clientY;

  event.preventDefault();  // Make nothing selectable
});

addEventListener("contextmenu", (event) => {
  event.preventDefault();    // NOTE comment to be able to acces dev tools on right click
});

var pointer_x = 0;
var pointer_y = 0;
export function get_pointer_x() {
  return pointer_x
}

export function get_pointer_y() {
  return pointer_y
}

export async function wait_for_pointer_move() {
  await getPromiseFromLeftButtonEvent(document, "mousemove")
}
