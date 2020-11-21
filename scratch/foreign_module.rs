// Scratch pad for figuring stuff out.

pub mod foreign_module {
    use crate::bindings::{self};
    use crate::{
        ForeignBindings, ForeignClass, ForeignClasses, ForeignMethod, WrenBuilder, WrenClass,
        WrenContext, WrenVm,
    };
    use std::{mem, os::raw::c_void};

    pub fn register(foreign: &mut ForeignBindings) {
        foreign.methods.0.insert(
            (
                "engine".to_string(),
                "Engine".to_string(),
                "log(_)".to_string(),
            ),
            ForeignMethod {
                is_static: false,
                arity: 1,
                sig: "".to_owned(),
                func: log,
            },
        );
        foreign.methods.0.insert(
            (
                "engine".to_string(),
                "Vector3".to_string(),
                "contents()".to_string(),
            ),
            ForeignMethod {
                is_static: false,
                arity: 0,
                sig: "".to_owned(),
                func: vector3_contents,
            },
        );

        let key = ("engine".to_string(), "Engine".to_string());
        foreign
            .classes
            .0
            .insert(key, ForeignClass { allocate, finalize });

        foreign.classes.0.insert(
            ("engine".to_string(), "Vector3".to_string()),
            ForeignClass {
                allocate: vector3_allocate,
                finalize: vector3_finalize,
            },
        );
    }

    /* ====== *
     * Engine *
     * ====== */

    struct Engine;

    impl WrenClass for Engine {
        const NAME: &'static str = "Engine";

        fn create(_: &WrenContext) -> Self {
            println!("Rust: Create Engine");
            Engine
        }
    }

    extern "C" fn log(vm: *mut bindings::WrenVM) {
        println!("Engine.log called");
    }

    extern "C" fn allocate(vm: *mut bindings::WrenVM) {
        println!("Engine allocating");

        let space: *mut Engine = unsafe {
            bindings::wrenSetSlotNewForeign(vm, 0, 0, mem::size_of::<Engine>() as usize) as _
        };

        let vm: &mut bindings::WrenVM = unsafe { vm.as_mut().unwrap() };
        let ctx = WrenContext::new(vm);
        let mut object = Engine::create(&ctx);

        mem::swap(unsafe { space.as_mut().unwrap() }, &mut object);
    }

    extern "C" fn finalize(vm: *mut c_void) {}

    /* ======= *
     * Vector3 *
     * ======= */

    #[derive(Debug)]
    struct Vector3 {
        x: f64,
        y: f64,
        z: f64,
    }

    impl WrenClass for Vector3 {
        const NAME: &'static str = "Vector3";

        fn create(_: &WrenContext) -> Self {
            println!("Rust: Create Vector3");
            Vector3 {
                x: 7.0,
                y: 11.0,
                z: 23.0,
            }
        }
    }

    extern "C" fn vector3_contents(vm: *mut bindings::WrenVM) {
        // Receiver type must be validated.
        //
        // If the class itself is not `foreign`, the receiver will be
        // type Unknown, and can't be cast to a Rust type.
        let slot_type = unsafe { bindings::wrenGetSlotType(vm, 0) };
        println!("Slot Type: {}", slot_type);
        if slot_type != bindings::WrenType_WREN_TYPE_FOREIGN {
            panic!("Rust method called with unknown receiver type.");
        }

        let vector3_ptr: *mut Vector3 = unsafe { bindings::wrenGetSlotForeign(vm, 0) as _ };
        let vector3 = unsafe { vector3_ptr.as_mut().unwrap() };
        println!("{:?}", vector3);
    }

    extern "C" fn vector3_allocate(vm: *mut bindings::WrenVM) {
        println!("Vector3 allocating");

        let space: *mut Vector3 = unsafe {
            bindings::wrenSetSlotNewForeign(vm, 0, 0, mem::size_of::<Vector3>() as usize) as _
        };

        let vm: &mut bindings::WrenVM = unsafe { vm.as_mut().unwrap() };
        let ctx = WrenContext::new(vm);
        let mut object = Vector3::create(&ctx);

        mem::swap(unsafe { space.as_mut().unwrap() }, &mut object);
    }

    extern "C" fn vector3_finalize(vm: *mut c_void) {}
}

