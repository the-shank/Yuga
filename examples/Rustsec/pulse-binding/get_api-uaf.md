## Potential use-after-free!
pulse-binding/src/mainloop/standard.rs:395:5: 395:66
```rust
pub fn get_api<'a>(&self) -> &'a ::mainloop::api::MainloopApi
```

`*(*(self)._inner.0.api)` is of type `MainloopApi` and outlives the lifetime corresponding to `'_`, 

It is (probably) returned as `*(ret)` which is of type `::mainloop::api::MainloopApi`, and outlives the lifetime corresponding to `'a`, . Here, `ret` denotes the value returned by the function.

The latter can be longer than the former, which could lead to use-after-free!

**Detailed report:**

`self` is of type `Mainloop`
```rust
pub struct Mainloop {
    /// The ref-counted inner data
    pub _inner: Rc<super::api::MainloopInner<MainloopInternal>>,
}
```
`*(self)._inner` is of type `Rc<super::api::MainloopInner<MainloopInternal>>`
```rust
pub struct MainloopInner<T>
    where T: MainloopInternalType
{
    /// An opaque main loop object
    pub ptr: *mut T,

    /// The abstract main loop API vtable for the GLIB main loop object. No need to free this API as
    /// it is owned by the loop and is destroyed when the loop is freed.
    pub api: *const MainloopApi,

    /// All implementations must provide a drop method, to be called from an actual drop call.
    pub dropfn: fn(&mut MainloopInner<T>),

    /// Whether or not the implementation supports monotonic based time events. (`true` if so).
    pub supports_rtclock: bool,
}
```
`Mainloop` has a custom `Drop` implementation.
```rust
fn drop(&mut self) {
        (self.dropfn)(self);
    }
```
`*(self)._inner.0.api` is of type `*const MainloopApi`

`ret` is of type `&'a ::mainloop::api::MainloopApi`


Here is the full body of the function:

```rust
pub fn get_api<'a>(&self) -> &'a ::mainloop::api::MainloopApi{
        let ptr = (*self._inner).api;
        assert_eq!(false, ptr.is_null());
        unsafe { &*ptr }
    }
```
