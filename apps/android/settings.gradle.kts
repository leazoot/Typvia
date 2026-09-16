// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

pluginManagement {
    repositories {
        google()
        mavenCentral()
        gradlePluginPortal()
    }
}

dependencyResolutionManagement {
    repositoriesMode.set(RepositoriesMode.FAIL_ON_PROJECT_REPOS)
    repositories {
        google()
        mavenCentral()
    }
}

rootProject.name = "Typvia"

// The design system and the typed wrapper over the Rust core. The app and —
// from the next stage — the IME both build on these, the same way every iOS
// product builds on TypviaKit.
include(":core-ffi")
include(":core-ui")
include(":app")

// The IME lives outside this directory because it predates the app: it was
// written while the mobile shell was a Tauri one, and it has been without a
// host since that shell was retired. Its own build file says as much. Giving
// it a root back is the first step of bringing it into the product; moving
// the sources is not, and is not done here.
include(":ime")
project(":ime").projectDir = file("../../native/android-ime")

// The share target, for the same reason and on the same terms: it predates
// this app, it writes into the app's own data dir, and it has been shipping
// nowhere since the shell that carried it was retired.
include(":share")
project(":share").projectDir = file("../../native/android-share-target")
