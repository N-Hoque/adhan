ARM_ARCH := "armv7"
arch_name := if ARM_ARCH == "armv7" { "armv7-unknown-linux-gnueabihf" } else { "aarch64-unknown-linux-gnu" } 

build-native:
    cargo build --profile size
build-arm:
    cross build --profile size --target {{arch_name}}
deploy-native-client user ip target: build-native
    rsync -vihP target/size/adhan {{user}}@{{ip}}:{{target}}
deploy-arm user ip target: build-arm
    rsync -vihP target/size/adhan {{user}}@{{ip}}:{{target}}