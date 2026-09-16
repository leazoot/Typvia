// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

// Formal Typvia IME library module. It ships inside the main app APK, where
// sharing the app's UID lets the InputMethodService read the KeyboardSnapshot
// directly, and it takes its AGP/Kotlin plugin versions from the including
// root project's buildscript classpath. No such root exists right now: the
// Tauri mobile shell that provided one was retired, and the native Android
// app that replaces it has not been built yet, so this module currently has
// no host to be included by.

plugins {
    id("com.android.library")
    id("org.jetbrains.kotlin.android")
}

// Resolved from this module's own location (native/android-ime) so the wiring
// does not depend on which Gradle root includes the module.
val repoRoot: File = projectDir.parentFile.parentFile

// The FFI is no longer produced here. `:core-ffi` builds it, owns the one
// task that does, and hands the bindings on — two modules generating the same
// files into the same directory is a build Gradle refuses to plan, and it was
// only ever this way because there was no `:core-ffi` to defer to.
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

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_11
        targetCompatibility = JavaVersion.VERSION_11
    }
    kotlinOptions {
        // Eleven, matching every other module the panel is built out of; the
        // design system it now reads is compiled at that level.
        jvmTarget = "11"
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

dependencies {
    // The bindings, the .so files and JNA all arrive through this one door.
    implementation(project(":core-ffi"))
    // The design system: the panel's palette, its type ladder and the four
    // band heights come from the same table the app reads, so the keyboard's
    // paper cannot drift from the app's without something failing.
    implementation(project(":core-ui"))
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
