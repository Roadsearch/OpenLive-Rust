plugins {
    kotlin("jvm") version "2.1.20"
}

kotlin {
    jvmToolchain(17)
    // Keep the production layout ready to copy into an Android app module,
    // while compiling the same paths in this standalone JVM test project.
    sourceSets {
        main {
            kotlin.srcDir("src/main/java")
        }
        test {
            kotlin.srcDir("src/test/java")
        }
    }
}

dependencies {
    testImplementation(kotlin("test-junit5"))
    testImplementation("org.junit.jupiter:junit-jupiter:5.11.4")
    testRuntimeOnly("org.junit.platform:junit-platform-launcher:1.11.4")
}

tasks.test {
    useJUnitPlatform()
    testLogging {
        events("passed", "skipped", "failed")
    }
}
