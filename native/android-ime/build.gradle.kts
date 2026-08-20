// Formal Typvia IME library module. Included by the committed
// apps/mobile/src-tauri/gen/android/settings.gradle so the InputMethodService
// ships inside the main app APK, where sharing the app's UID lets it read the
// KeyboardSnapshot directly. AGP/Kotlin plugin versions come from that root
// project's buildscript classpath.

plugins {
    id("com.android.library")
    id("org.jetbrains.kotlin.android")
}

// Resolved from this module's own location (native/android-ime) so the wiring
// does not depend on which Gradle root includes the module.
val repoRoot: File = projectDir.parentFile.parentFile
val ffiOut = File(repoRoot, "target/mobile-ffi-android")
val hostDylib = File(repoRoot, "target/release/libtypvia_mobile_ffi.dylib")

// The module consumes the mobile-ffi build products (generated Kotlin bindings
// + per-ABI .so + host dylib for JVM unit tests). They are never committed;
// this task regenerates them when missing or stale so a clean checkout
// builds reproducibly. Inputs cover the FFI crate only — a deeper workspace
// change requires a manual `crates/mobile-ffi/build-android.sh` run.
val buildMobileFfi = tasks.register<Exec>("buildMobileFfi") {
    workingDir = repoRoot
    commandLine("bash", "crates/mobile-ffi/build-android.sh")
    inputs.files(
        fileTree(File(repoRoot, "crates/mobile-ffi/src")),
        File(repoRoot, "crates/mobile-ffi/Cargo.toml"),
        File(repoRoot, "crates/mobile-ffi/build-android.sh"),
        File(repoRoot, "crates/mobile-ffi/smoke/Main.kt"),
    )
    outputs.dir(File(ffiOut, "generated"))
    outputs.dir(File(ffiOut, "jniLibs"))
    outputs.file(hostDylib)
}

android {
    namespace = "dev.typvia.ime"
    compileSdk = 36

    defaultConfig {
        // Product OS baseline: Android 9.0 / API 28, same as app.
        minSdk = 28
        consumerProguardFiles("consumer-rules.pro")
        // Connected closed-loop test: the self-instrumenting test
        // APK carries the IME service plus a test-only host activity.
        testInstrumentationRunner = "androidx.test.runner.AndroidJUnitRunner"
    }

    sourceSets.getByName("main") {
        java.srcDir(File(ffiOut, "generated"))
        jniLibs.srcDirs(File(ffiOut, "jniLibs"))
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_1_8
        targetCompatibility = JavaVersion.VERSION_1_8
    }
    kotlinOptions {
        jvmTarget = "1.8"
    }

    testOptions {
        unitTests.all {
            // Host-JVM unit tests exercise the real FFI through the host
            // dylib, the same loading path as the mobile-ffi smoke test.
            it.systemProperty(
                "jna.library.path",
                File(repoRoot, "target/release").absolutePath,
            )
        }
    }
}

tasks.named("preBuild") {
    dependsOn(buildMobileFfi)
}

dependencies {
    // UniFFI-generated Kotlin requires JNA at runtime; it is taken under the
    // Apache-2.0 option of its Apache-2.0/LGPL-2.1 dual license, which is
    // MPL-2.0 compatible. The @aar variant carries Android libjnidispatch.
    implementation("net.java.dev.jna:jna:5.17.0@aar")
    testImplementation("junit:junit:4.13.2")
    // Plain jar for host-JVM unit tests (loads the host dylib, not the .so).
    testImplementation("net.java.dev.jna:jna:5.17.0")
    // Instrumentation closed loop, test-only: AndroidX Test
    // runner/core/junit drive the host activity, UiAutomator crosses into
    // the IME window (an Espresso view scope cannot reach another process's
    // window). All Apache-2.0; none ship in any product APK.
    androidTestImplementation("androidx.test:runner:1.6.2")
    androidTestImplementation("androidx.test:core:1.6.1")
    androidTestImplementation("androidx.test.ext:junit:1.2.1")
    androidTestImplementation("androidx.test.uiautomator:uiautomator:2.3.0")
}
