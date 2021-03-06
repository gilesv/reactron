let context;
let workLoop;

export default {
  render() {
    throw new Error("Reactron: 'render' used before loading wasm module");
  },

  useState() {
    throw new Error("Reactron: 'useState' used before loading wasm module");
  },

  createElement() {
    throw new Error("Reactron: 'createElement' used before loading wasm module");
  },

  load() {
    return import("../pkg/reactron_bg.js").then((glue) => {
      context = glue.get_context();

      workLoop = (deadline) => {
        context = glue.work_loop(context, deadline.didTimeout);
        window.requestIdleCallback(workLoop);
      };

      this.render = (element, parentDom) => {
        context = glue.render(context, element, parentDom);
        window.requestIdleCallback(workLoop);
      };

      this.useState = (initialValue) => {
        let result = glue.use_state(context, initialValue);
        return result;
      };

      this.createElement = (type, props, ...rawChildren) => {
        props = props || {};
        let children = rawChildren
          .flat()
          .filter((x) => x)
          .map((x) => {
            return typeof x === "string"
              ? glue.create_text_element(x)
              : x;
          });

        let isFunctionalComponent = typeof type === "function";

        if (isFunctionalComponent) {
          props.children = children;
          return glue.create_functional_component(type, props);
        } else {
          let elementProps = glue.create_props(
            props ? props.className : null,
            props ? props.nodeValue: null,
            props ? props.onClick : null,
            props ? props.onChange : null,
            props ? props.onBlur : null,
            props ? props.onKeyDown : null,
            props ? props.type : null,
            props ? props.value : null,
            props ? props.checked : null,
            props ? props.placeholder : null,
          );
          return glue.create_element(type, elementProps, children);
        }
      };
    });
  }
}
