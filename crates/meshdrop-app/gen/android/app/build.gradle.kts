import java.util.Properties

plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.android")
    id("rust")
}

val tauriProperties = Properties().apply {
    val propFile = file("tauri.properties")
    if (propFile.exists()) {
        propFile.inputStream().use { load(it) }
    }
}

// CLI-generated properties can omit the version. Keep Cargo as the only source.
val workspaceVersion = rootProject.file("../../../../Cargo.toml").readLines()
    .single { it.trimStart().startsWith("version = ") }
    .substringAfter('"').substringBefore('"')
val versionParts = workspaceVersion.split('.').map { it.toInt() }
require(versionParts.size == 3 && versionParts.all { it in 0..999 }) {
    "Android requires a three-part workspace version with components below 1000"
}
val workspaceVersionCode = versionParts.fold(0) { code, part -> code * 1000 + part }

android {
    compileSdk = 36
    namespace = "org.meshdrop.desktop"
    defaultConfig {
        manifestPlaceholders["usesCleartextTraffic"] = "false"
        applicationId = "org.meshdrop.desktop"
        minSdk = 24
        targetSdk = 36
        versionCode = workspaceVersionCode
        versionName = workspaceVersion
    }
    // Signing material is supplied only through the environment, never properties files.
    val releaseStore = System.getenv("MESHDROP_KEYSTORE_PATH")
    gradle.taskGraph.whenReady {
        require(releaseStore != null || allTasks.none {
            it.project == project && it.name.endsWith("Release")
        }) { "Release tasks require MESHDROP_KEYSTORE_PATH and signing environment variables" }
    }
    signingConfigs {
        if (releaseStore != null) {
            create("release") {
                storeFile = file(releaseStore)
                storePassword = requireNotNull(System.getenv("MESHDROP_KEYSTORE_PASSWORD"))
                keyAlias = requireNotNull(System.getenv("MESHDROP_KEY_ALIAS"))
                keyPassword = requireNotNull(System.getenv("MESHDROP_KEY_PASSWORD"))
            }
        }
    }
    buildTypes {
        getByName("debug") {
            manifestPlaceholders["usesCleartextTraffic"] = "true"
            isDebuggable = true
            isJniDebuggable = true
            isMinifyEnabled = false
            packaging {                jniLibs.keepDebugSymbols.add("*/arm64-v8a/*.so")
                jniLibs.keepDebugSymbols.add("*/armeabi-v7a/*.so")
                jniLibs.keepDebugSymbols.add("*/x86/*.so")
                jniLibs.keepDebugSymbols.add("*/x86_64/*.so")
            }
        }
        getByName("release") {
            if (releaseStore != null) {
                signingConfig = signingConfigs.getByName("release")
            }
            isMinifyEnabled = true
            proguardFiles(
                *fileTree(".") { include("**/*.pro") }
                    .plus(getDefaultProguardFile("proguard-android-optimize.txt"))
                    .toList().toTypedArray()
            )
        }
    }
    kotlinOptions {
        jvmTarget = "1.8"
    }
    buildFeatures {
        buildConfig = true
    }
}

rust {
    rootDirRel = "../../../"
}

dependencies {
    implementation("androidx.webkit:webkit:1.14.0")
    implementation("androidx.appcompat:appcompat:1.7.1")
    implementation("androidx.activity:activity-ktx:1.10.1")
    implementation("com.google.android.material:material:1.12.0")
    implementation("androidx.lifecycle:lifecycle-process:2.10.0")
    testImplementation("junit:junit:4.13.2")
    androidTestImplementation("androidx.test.ext:junit:1.1.4")
    androidTestImplementation("androidx.test.espresso:espresso-core:3.5.0")
}

apply(from = "tauri.build.gradle.kts")