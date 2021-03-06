use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{Element as HTMLElement, Text as HTMLText, Window, Document};
use std::cell::RefCell;
use std::rc::Rc;
use std::mem;

mod element;
mod fiber;
mod constants;
use element::{Element, ElementProps, Node};
use fiber::{Fiber, FiberCell, FiberEffect, FiberParentIterator};
use constants::{TEXT_ELEMENT, FIBER_ROOT, FIBER_FUNCTIONAL};

#[wasm_bindgen]
pub struct Context {
    wip_root: Option<FiberCell>,
    current_root: Option<FiberCell>,
    next_unit_of_work: Option<FiberCell>,
    wip_functional_fiber: Option<FiberCell>,
    effects: Vec<FiberCell>,
    document: Document
}

impl Context {
    pub fn new() -> Self {
        let window: Window = web_sys::window().unwrap();
        let document: Document = window.document().unwrap();

        Context {
            wip_root: None,
            current_root: None,
            next_unit_of_work: None,
            wip_functional_fiber: None,
            effects: Vec::new(),
            document
        }
    }

    fn add_effect(&mut self, effect: FiberCell) {
        self.effects.push(effect);
    }

    pub fn from_ptr(ptr: *mut Context) -> Box<Context> {
        unsafe { Box::from_raw(ptr) }
    }

    fn work_loop(&mut self, did_timeout: bool) -> Result<(), JsValue> {
        let mut no_next_unit_of_work = self.next_unit_of_work.is_none();

        loop {
            if did_timeout || no_next_unit_of_work {
                break;
            }

            let wip_fiber = Rc::clone(&self.next_unit_of_work.as_ref().unwrap());
            self.next_unit_of_work = self.perform_unit_of_work(wip_fiber);
            
            no_next_unit_of_work = self.next_unit_of_work.is_none();
        }

        if no_next_unit_of_work && self.wip_root.is_some() {
            self.commit_root()?;
        }

        Ok(())
    }

    fn perform_unit_of_work(&mut self, wip_fiber: FiberCell) -> Option<FiberCell> {
        let is_functional_tree = wip_fiber.borrow().is_functional_tree();

        if is_functional_tree {
            let fiber = wip_fiber.borrow();

            let func = Rc::clone(&fiber.component_function().unwrap());
            let props = Rc::clone(&fiber.component_function_props().unwrap());

            // Drop the borrow so it can be borrowed from 'use_state'
            mem::drop(fiber);

            self.wip_functional_fiber = Some(Rc::clone(&wip_fiber));
            let child = self.execute_function_component(func, props);
            self.wip_functional_fiber = None;

            let mut fiber = wip_fiber.borrow_mut();

            if let Some(child) = child {
                let children_vec = vec![child];
                fiber.set_element_children(Some(Rc::new(RefCell::new(children_vec))));
            }

            self.reconcile_children(&wip_fiber, &mut fiber);
        } else {
            let mut fiber = wip_fiber.borrow_mut();

            if fiber.dom_node().is_none() {
                let dom_node = self.create_dom_node(&fiber);

                fiber.set_dom_node(Rc::new(RefCell::new(dom_node)));
            }

            self.reconcile_children(&wip_fiber, &mut fiber);
        }

        let fiber = wip_fiber.borrow_mut();

        // Add to effect list
        if fiber.effect_tag().is_some() {
            self.add_effect(Rc::clone(&wip_fiber));
        }

        // If fiber has a child, make it the next unit of work
        if let Some(fiber_child) = &fiber.child() {
            // console_log!("{} has a child", &fiber.element_type());
            return Some(Rc::clone(fiber_child));

        // ...or if it has a sibling, make it the next unit of work
        } else if let Some(fiber_sibling) = &fiber.sibling() {
            // console_log!("{} has a sibling", &fiber.element_type());
            return Some(Rc::clone(fiber_sibling));

        // Otherwise look for the closest parent's sibling
        } else {
            // console_log!("{} has no child or sibling. coming back to parents", &fiber.element_type());

            // Drop the mutable borrow to avoid crashing when looping through the parents
            mem::drop(fiber);

            for parent in wip_fiber.parents() {
                if let Some(parent_sibling) = &parent.borrow().sibling() {
                    return Some(Rc::clone(parent_sibling));
                }
            }
        }

        return None;
    }

    fn execute_function_component(
        &self,
        func: Rc<js_sys::Function>,
        props: Rc<JsValue>
    ) -> Option<Box<Element>> {
        func.call1(&JsValue::null(), &props)
            .unwrap()
            .as_f64()
            .map(|child_ptr| Element::from_ptr(child_ptr as u32 as *mut Element))
    }

    fn create_dom_node(&self, fiber: &Fiber) -> Node {
        let props = fiber.props().unwrap();

        if fiber.is_text_fiber() {
            let node: HTMLText = self.document.create_text_node(&props.node_value().unwrap());

            Node::Text(node)
        } else {
            let node = self.document.create_element(fiber.element_type()).unwrap();
            self.update_dom_node(&node, None, &props);

            Node::Element(node)
        }
    }

    fn update_dom_node(&self, dom_node: &HTMLElement, prev_props: Option<&Box<ElementProps>>, next_props: &Box<ElementProps>) {
        let prev_class_name = prev_props.and_then(|p| p.class_name());
        let next_class_name = next_props.class_name();

        let prev_on_click = prev_props.and_then(|p| p.on_click());
        let next_on_click = next_props.on_click();
        let prev_on_change = prev_props.and_then(|p| p.on_change());
        let next_on_change = next_props.on_change();
        let prev_on_blur = prev_props.and_then(|p| p.on_blur());
        let next_on_blur = next_props.on_blur();
        let prev_on_keydown = prev_props.and_then(|p| p.on_keydown());
        let next_on_keydown = next_props.on_keydown();
        let prev_input_type = prev_props.and_then(|p| p.input_type());
        let next_input_type = next_props.input_type();
        let prev_input_value = prev_props.and_then(|p| p.input_value());
        let next_input_value = next_props.input_value();
        let prev_input_placeholder = prev_props.and_then(|p| p.input_placeholder());
        let next_input_placeholder = next_props.input_placeholder();
        let prev_input_checked = prev_props.and_then(|p| p.input_checked());
        let next_input_checked = next_props.input_checked();

        // Class name
        match (prev_class_name, next_class_name) {
            (Some(prev), Some(next)) => {
                if *prev != *next {
                    dom_node.set_class_name(next);
                }
            },
            (None, Some(next)) => {
                dom_node.set_class_name(next);
            },
            (_, _) => {}
        }

        // Input type
        match (prev_input_type, next_input_type) {
            (None, Some(next)) => {
                dom_node.unchecked_ref::<web_sys::HtmlInputElement>()
                    .set_type(next);
            },
            (_, _) => {}
        }

        // Input value
        match (prev_input_value, next_input_value) {
            (_, Some(next)) => {
                dom_node.unchecked_ref::<web_sys::HtmlInputElement>()
                    .set_value(next);
            },
            (_, _) => {}
        }

        // Input checked
        match (prev_input_checked, next_input_checked) {
            (_, Some(next)) => {
                dom_node.unchecked_ref::<web_sys::HtmlInputElement>()
                    .set_checked(next);
            },
            (_, _) => {}
        }

        // Input placeholder
        match (prev_input_placeholder, next_input_placeholder) {
            (Some(prev), Some(next)) => {
                if *prev != *next {
                    dom_node.unchecked_ref::<web_sys::HtmlInputElement>()
                        .set_placeholder(next);
                }
            },
            (None, Some(next)) => {
                dom_node.unchecked_ref::<web_sys::HtmlInputElement>()
                    .set_placeholder(next);
            }
            (_, _) => {}
        }

        self.update_listener(
            "click",
            &prev_on_click,
            &next_on_click,
            &dom_node
        );

        self.update_listener(
            "change",
            &prev_on_change,
            &next_on_change,
            &dom_node
        );

        self.update_listener(
            "blur",
            &prev_on_blur,
            &next_on_blur,
            &dom_node
        );

        self.update_listener(
            "keydown",
            &prev_on_keydown,
            &next_on_keydown,
            &dom_node
        );
    }

    fn update_listener(
        &self,
        event_type: &str,
        prev_listener: &Option<&js_sys::Function>,
        next_listener: &Option<&js_sys::Function>,
        dom_node: &HTMLElement,
    ) {
        match (prev_listener, next_listener) {
            (None, Some(callback)) => {
                dom_node.add_event_listener_with_callback(event_type, callback).unwrap();
            },
            (Some(callback), None) => {
                dom_node.remove_event_listener_with_callback(event_type, callback).unwrap();
            },
            (Some(prev_callback), Some(next_callback)) => {
                dom_node.remove_event_listener_with_callback(event_type, prev_callback).unwrap();
                dom_node.add_event_listener_with_callback(event_type, next_callback).unwrap();
            },
            (_, _) => {}
        }
    }

    fn update_dom_text(&self, text_node: &HTMLText, prev_props: Option<&Box<ElementProps>>, next_props: &Box<ElementProps>) {
        let prev_value = prev_props.and_then(|p| p.node_value());
        let next_value = next_props.node_value();

        match (prev_value, next_value) {
            (Some(prev), Some(next)) => {
                if *prev != *next {
                    text_node.set_node_value(Some(next));
                }
            },
            (None, Some(next)) => {
                text_node.set_node_value(Some(next));
            },
            (_, _) => {}
        }
    }

    fn reconcile_children(&mut self, wip_unit: &FiberCell, fiber: &mut Fiber) {
        let children = fiber.element_children().as_ref();
        let children_len = children.map_or(0, |children| children.borrow().len());

        let mut i = 0;
        let mut previous_sibling: Option<FiberCell> = None;
        let mut first_child_fiber: Option<FiberCell> = None;

        let mut old_child_fiber = fiber.alternate().map_or(None, |alternate| {
            alternate.borrow()
                .child()
                .as_ref()
                .map_or(None, |child| {
                    Some(Rc::clone(child))
                })
        });

        while i < children_len || old_child_fiber.as_ref().is_some() {
            let mut children = children.unwrap().borrow_mut();
            let child_element = children.get_mut(i);

            let has_same_type = old_child_fiber.as_ref().map_or(false, |old_child| {
                child_element.as_ref().map_or(false, |child| {
                    *old_child.borrow().element_type() == *child.element_type()
                })
            });

            let child_fiber = child_element.map_or(None, |child_element| {
                // Generate a new Fiber for the updated node
                let mut child_fiber = if has_same_type {
                    let alternate_child = old_child_fiber.as_ref().unwrap();
                    let mut child_fiber = Fiber::new(&alternate_child.borrow().element_type());

                    child_fiber.set_props(child_element.props_mut().take());

                    let element_children = child_element.children_mut().take().map(|children| {
                        Rc::new(RefCell::new(children))
                    });

                    child_fiber.set_element_children(element_children);

                    // relate to alternate
                    child_fiber.set_alternate(Rc::clone(&alternate_child));

                    // set existing dom node
                    if let Some(old_child_node) = alternate_child.borrow().dom_node() {
                        child_fiber.set_dom_node(Rc::clone(old_child_node));
                    }

                    // relate to parent (current fiber)
                    child_fiber.set_parent(Rc::clone(wip_unit));

                    // effect
                    if !child_fiber.is_functional_tree() {
                        if let Some(old_props) = alternate_child.borrow().props() {
                            if child_fiber.has_props_changed(old_props) {
                                child_fiber.set_effect_tag(FiberEffect::Update);
                                // console_log!("added UPDATE effect for {}", &child_fiber.element_type());
                            }
                        }
                    }

                    Some(child_fiber)
                } else {
                    let mut child_fiber = Fiber::new(&child_element.element_type());

                    child_fiber.set_props(child_element.props_mut().take());
                    let element_children = child_element.children_mut().take().map(|children| {
                        Rc::new(RefCell::new(children))
                    });
                    child_fiber.set_element_children(element_children);

                    // relate to parent (current fiber)
                    child_fiber.set_parent(Rc::clone(wip_unit));

                    // effect
                    if !child_fiber.is_functional_tree() {
                        child_fiber.set_effect_tag(FiberEffect::Placement);
                        // console_log!("added PLACEMENT effect for {}", &child_fiber.element_type());
                    }

                    Some(child_fiber)
                };

                child_fiber.as_mut().map(|child_fiber| {
                    if child_fiber.is_functional_tree() {
                        let func = child_element.component_function().unwrap();
                        let props = child_element.component_function_props().unwrap();

                        child_fiber.set_component_function(Some(Rc::clone(&func)));
                        child_fiber.set_component_function_props(Some(Rc::clone(&props)));
                        child_fiber.set_hooks(Some(vec![]));
                    }
                });

                child_fiber
            });

            if let Some(old_child_fiber) = old_child_fiber.as_ref() {
                if !has_same_type {
                    old_child_fiber.borrow_mut().set_effect_tag(FiberEffect::Deletion);
                    self.add_effect(Rc::clone(&old_child_fiber));
                    // console_log!("added deletion effect for {}", old_child_fiber.borrow().element_type());
                }
            }

            if old_child_fiber.is_some() {
                let old_child_sibling = {
                    let old_child = old_child_fiber.as_ref().unwrap().borrow();
                    if let Some(sibling) = old_child.sibling() {
                        Some(Rc::clone(sibling))
                    } else {
                        None
                    }
                };

                old_child_fiber = old_child_sibling;
            }

            
            if let Some(child_fiber) = child_fiber {
                let child_fiber = Rc::new(RefCell::new(Box::new(child_fiber)));

                if i == 0 {
                    first_child_fiber = Some(Rc::clone(&child_fiber));
                } else {
                    if let Some(previous_sibling) = previous_sibling {
                        previous_sibling.borrow_mut().set_sibling(Rc::clone(&child_fiber));
                    }
                }
                
                previous_sibling = Some(Rc::clone(&child_fiber));
                i += 1;
            }

        }

        if let Some(child) = first_child_fiber {
            fiber.set_child(child);
        }
    }

    fn commit_root(&mut self) -> Result<(), JsValue> {
        if self.wip_root.is_some() {
            let wip_root_fiber = self.wip_root.as_ref().unwrap();

            for effect in &self.effects {
                self.commit_work(&effect)?;
            }

            self.effects.clear();

            self.current_root = Some(Rc::clone(wip_root_fiber));
            self.wip_root = None;
        }

        Ok(())
    }

    fn commit_work(&self, fiber: &FiberCell) -> Result<(), JsValue> {
        match fiber.borrow().effect_tag() {
            Some(FiberEffect::Placement) => {
                let mut parent_dom_node = None;

                for parent in fiber.parents() {
                    let parent = parent.borrow();

                    if let Some(dom_node) = parent.dom_node() {
                        parent_dom_node = Some(Rc::clone(dom_node));
                        break;
                    }
                }

                // console_log!("executing PLACEMENT for {}", fiber.borrow().element_type());

                self.commit_node_append(&fiber, parent_dom_node)?;
            },
            Some(FiberEffect::Update) => {
                // console_log!("executing UPDATE for {}", fiber.borrow().element_type());
                self.commit_node_update(&fiber)?;
            },
            Some(FiberEffect::Deletion) => {
                // console_log!("executing DELETION for {}", fiber.borrow().element_type());

                // This can break if we have a functional component that returns another functional component
                // without an element in between. An iterator through all children of a fiber to find
                // the next DOM element would be fine but for my current case simply getting the first child is quicker.
                if fiber.borrow().is_functional_tree() {
                    let fiber_borrowed = fiber.borrow();

                    if let Some(child) = fiber_borrowed.child().as_ref() {
                        self.commit_node_deletion(&child)?;
                    } else {
                        self.commit_node_deletion(&fiber)?;
                    }
                } else {
                    self.commit_node_deletion(&fiber)?;
                }

            },
            None => {}
        }

        Ok(())
    }

    fn commit_node_append(&self, fiber: &FiberCell, parent_dom_node: Option<Rc<RefCell<Node>>>) -> Result<(), JsValue> {
        let has_dom_node = fiber.borrow().dom_node().is_some();
        let has_parent_node = parent_dom_node.is_some();

        if has_dom_node && has_parent_node {
            let fiber = fiber.borrow();
            let dom_node = fiber.dom_node().unwrap();
            let parent_node = parent_dom_node.unwrap();

            let dom_node = &*dom_node.borrow();
            let parent_node = &*parent_node.borrow();

            match (parent_node, dom_node) {
                // Append HTML element
                (Node::Element(parent), Node::Element(child)) => {
                    parent.append_child(&child)?;
                },

                // Append text node
                (Node::Element(parent), Node::Text(text)) => {
                    parent.append_child(&text)?;
                }
                _ => {}
            }
        }

        Ok(())
    }

    fn commit_node_update(&self, fiber: &FiberCell) -> Result<(), JsValue> {
        let fiber = fiber.borrow();

        if let Some(dom_node) = fiber.dom_node() {
            if let Some(alternate) = fiber.alternate() {
                let alternate = alternate.borrow();
                let prev_props = alternate.props();
                let next_props = fiber.props().unwrap();
                let node= &*dom_node.borrow();

                match node {
                    Node::Element(node) => {
                        self.update_dom_node(
                            &node,
                            prev_props,
                            next_props
                        );
                    },
                    Node::Text(text) => {
                        self.update_dom_text(
                            text,
                            prev_props,
                            next_props
                        );
                    }
                }
            }
        }

        Ok(())
    }

    fn commit_node_deletion(&self, fiber: &FiberCell) -> Result<(), JsValue> {
        if let Some(dom_node) = fiber.borrow().dom_node() {
            match &*dom_node.borrow() {
                Node::Element(node) => {
                    node.remove();
                },
                Node::Text(text) => {
                    text.remove();
                }
            }
        }

        Ok(())
    }
}

#[wasm_bindgen]
pub fn get_context() -> *mut Context {
    let context = Box::new(Context::new());
    Box::into_raw(context)
}

#[wasm_bindgen]
pub fn render(context_ptr: *mut Context, element_ptr: *mut Element, container: HTMLElement) -> *mut Context {
    let mut context = Context::from_ptr(context_ptr);
    let element = Element::from_ptr(element_ptr);

    // Create the Root fiber
    let mut root = Fiber::new_root();
    
    // The root element will be the Root fiber's only child
    let mut children = Vec::with_capacity(1);
    children.push(element);
    root.set_element_children(Some(Rc::new(RefCell::new(children))));

    // Store the container HTML element
    root.set_dom_node(Rc::new(RefCell::new(Node::Element(container))));

    // Set the current root as the alternate root
    if let Some(current_root) = context.current_root.as_ref() {
        root.set_alternate(Rc::clone(current_root));
    }

    // Make it the Work in Progress Root and the Next Unit of Work
    let root = Rc::new(RefCell::new(Box::new(root)));
    context.wip_root = Some(Rc::clone(&root));
    context.next_unit_of_work = Some(Rc::clone(&root));

    Box::into_raw(context)
}


#[wasm_bindgen]
pub fn work_loop(context_ptr: *mut Context, did_timeout: bool) -> *mut Context {
    let mut context = Context::from_ptr(context_ptr);

    context.work_loop(did_timeout).unwrap();

    Box::into_raw(context)
}

#[wasm_bindgen]
pub fn use_state(context_ptr: *mut Context, initial_value: JsValue) -> Box<[JsValue]> {
    let context = Context::from_ptr(context_ptr.clone());
    let wip_fiber = context.wip_functional_fiber.as_ref().unwrap();
    let mut fiber = wip_fiber.borrow_mut();

    let old_state = fiber.alternate().map_or(None, |alternate| {
        let alternate = alternate.borrow();
        let hook = alternate.get_hook_at(fiber.hook_idx() as usize);

        hook.map_or(None, |hook| {
            Some(hook.borrow().clone())
        })
    });

    let current_state = if old_state.is_some() {
        old_state.unwrap()
    } else {
        initial_value
    };

    let new_hook = Rc::new(RefCell::new(current_state.clone()));

    fiber.add_hook(Rc::clone(&new_hook));

    let set_state = Closure::wrap(Box::new(move |new_state: JsValue| {
        *new_hook.borrow_mut() = new_state;
        let mut context = Context::from_ptr(context_ptr);

        let current_root = context.current_root.as_ref();
        let mut root = Fiber::new_root();
        
        current_root.map(|current_root| {
            root.set_alternate(Rc::clone(current_root));

            let current_root = current_root.borrow();

            if let Some(children) = current_root.element_children().as_ref() {
                root.set_element_children(Some(Rc::clone(&children)));
            }

            // Store the container HTML element
            if let Some(dom_node) = current_root.dom_node().as_ref() {
                root.set_dom_node(Rc::clone(dom_node));
            }
        });

        // Make it the Work in Progress Root and the Next Unit of Work
        let root = Rc::new(RefCell::new(Box::new(root)));
        context.wip_root = Some(Rc::clone(&root));
        context.next_unit_of_work = Some(Rc::clone(&root));

        Box::into_raw(context);
    }) as Box<dyn FnMut(JsValue)>).into_js_value();

    fiber.incr_hook_idx();
    mem::drop(fiber);
    mem::drop(wip_fiber);

    Box::into_raw(context);

    vec![current_state, set_state].into_boxed_slice()
}
