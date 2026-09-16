// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

// The typed channel into the shared Rust core, and nothing else.
//
// It is its own module because three products need it — the app, the IME and
// (later) the share target — and because keeping it apart from the design
// system means a screen cannot quietly acquire a database dependency by being
// in the same module as one.

plugins {
    id("com.android.library")
    id("org.jetbrains.kotlin.android")
}

val repoRoot: File = projectDir.parentFile.parentFile.parentFile
val ffiOut = File(repoRoot, "target/mobile-ffi-android")

// The build products the FFI script leaves behind: generated Kotlin bindings
// and one .so per ABI. They are never committed, so a clean checkout has to
// be able to produce them — this task does, and declares its inputs so an
// unchanged crate does not pay for a rebuild.
val buildMobileFfi = tasks.register<Exec>("buildMobileFfi") {
    workingDir = repoRoot
    commandLine("bash", "crates/mobile-ffi/build-android.sh")
    // The script's own Kotlin smoke starts a second Gradle build. From inside
    // this one that is both slow and redundant: the bindings it would check
    // are compiled by :core-ffi a few tasks from here.
    environment("TYPVIA_FFI_SMOKE", "skip")
    inputs.files(
        fileTree(File(repoRoot, "crates/mobile-ffi/src")),
        File(repoRoot, "crates/mobile-ffi/Cargo.toml"),
        File(repoRoot, "crates/mobile-ffi/build-android.sh"),
    )
    outputs.dir(File(ffiOut, "generated"))
    outputs.dir(File(ffiOut, "jniLibs"))
}

android {
    namespace = "dev.typvia.mobile.ffi"
    compileSdk = 36

    defaultConfig {
        // Product OS baseline: Android 9.0 / API 28.
        minSdk = 28
    }

    sourceSets.getByName("main") {
        java.srcDir(File(ffiOut, "generated"))
        jniLibs.srcDirs(File(ffiOut, "jniLibs"))
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_11
        targetCompatibility = JavaVersion.VERSION_11
    }
    kotlinOptions {
        jvmTarget = "11"
    }
}

tasks.named("preBuild") {
    dependsOn(buildMobileFfi)
}

dependencies {
    // UniFFI-generated Kotlin needs JNA at runtime; taken under the Apache-2.0
    // option of its Apache-2.0/LGPL-2.1 dual license, which MPL-2.0 allows.
    // The @aar variant carries Android's libjnidispatch.
    api("net.java.dev.jna:jna:5.17.0@aar")
    implementation("org.jetbrains.kotlinx:kotlinx-coroutines-android:1.9.0")
    testImplementation("junit:junit:4.13.2")
}
