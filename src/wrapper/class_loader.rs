use std::sync::Mutex;

use errors::*;

use JNIEnv;

use objects::{GlobalRef, JObject, JClass, JValue};

use strings::JNIString;

use lazy_static::lazy_static;

/// The `loadClass` function name.
const LOAD_CLASS: &str = "loadClass";
/// The `loadClass` signature.
const LOAD_CLASS_SIG: &str = "(Ljava/lang/String;)Ljava/lang/Class;";

lazy_static! {
    /// The global class loader instance.
    static ref CLASS_LOADER: Mutex<Option<GlobalRef>> = Mutex::default();
}

// Review: I would also describe the effects of this change on any subsequent find_class invocations.
/// Register the given object as the global class loader instance.
pub fn register_class_loader<'a>(env: &JNIEnv<'a>, class_loader: JObject<'a>) -> Result<()> {
    /*
    Review: I'd suggest a little clearer way: env.is_instance_of()
    However, it would also require a not null check.
    */
    // Check that the `loadClass` function is present.
    env.get_method_id(class_loader, LOAD_CLASS, LOAD_CLASS_SIG)?;

    *CLASS_LOADER.lock().unwrap() = Some(env.new_global_ref(class_loader)?);

    Ok(())
}

/// Unregister the global class loader instance.
pub fn unregister_class_loader() {
    *CLASS_LOADER.lock().unwrap() = None;
}

/// Look up a class by name.
///
/// Either it uses the registered `CLASS_LOADER` or it falls back to use the
/// JNI env function `FindClass`.
pub(crate) fn load_class<'a>(env: &JNIEnv<'a>, name: JNIString) -> Result<JClass<'a>> {
    /*
Review: This code will serialize all class lookups. I think we must ensure that normal find_class
clients do not pay any price for this feature and are not serialized.

Possibly, one way to achieve that is to acquire the lock only to read the optional because
there is no requirement of a mutual exclusion between the class lookups and
`register_class_loader`/`unregister_class_loader`. GlobalRefs can be cloned and safely accessed
from multiple threads — the last one dropping it will destroy it.
    */
    match *CLASS_LOADER.lock().unwrap() {
        Some(ref class_loader) => {
            let name = env.new_string(name)?;
            let res = env.call_method(
                class_loader.as_obj(),
                /* Review:
As this is library code, I think it *must* cache the method id of `Classloader#loadClass`
and use `call_method_unchecked` for adequate performance. Otherwise each find_class (that may
be implicit) will also entail (implicit here) `get_object_class` + `get_method_id`.
                */
                LOAD_CLASS,
                LOAD_CLASS_SIG,
                &[JValue::Object(name.into())]
            )?;
            res.l().map(Into::into)
        },
        None => {
            let class = jni_non_null_call!(env.get_native_interface(), FindClass, name.as_ptr());
            Ok(class)
        }
    }
}