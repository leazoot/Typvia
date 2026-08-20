# Consumed by the app module's minified release build. JNA dispatches over
# reflection and the UniFFI bindings are looked up by name; neither survives
# renaming.
-keep class com.sun.jna.** { *; }
-keep class uniffi.typvia_mobile_ffi.** { *; }
-dontwarn java.awt.**
