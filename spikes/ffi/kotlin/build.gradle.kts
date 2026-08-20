// SPIKE: Kotlin/JVM harness for the UniFFI + JNA binding path.
// JVM stands in for Android here — the generated Kotlin binding is identical;
// the Android-target .so cross-build was already proven separately.
plugins {
    kotlin("jvm") version "2.1.20"
    application
}

repositories {
    mavenCentral()
}

dependencies {
    implementation("net.java.dev.jna:jna:5.17.0")
}

sourceSets {
    main {
        kotlin.srcDir("generated")
    }
}

application {
    mainClass.set("MainKt")
    applicationDefaultJvmArgs = listOf("-Djna.library.path=${projectDir}/../rust/target/debug")
}
