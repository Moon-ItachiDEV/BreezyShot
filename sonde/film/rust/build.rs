// S2 point 8 : les exemples sont liés comme le serait l'app de la 0.7, les DLL de Media
// Foundation, de DXGI et de Direct3D chargées à la demande (/DELAYLOAD, comme comctl32
// dans app/coeur/build.rs), plus la DLL inventée `bzs-absente.dll` de l'exemple `charge`
// (8c). Le binaire `charge-temoin` (src/bin/, même code que l'exemple `charge`, sans la
// DLL inventée) ne l'est pas : c'est le témoin qui montre que la mesure voit la différence.
// Un /DELAYLOAD d'une DLL dont l'exécutable n'importe rien donne l'avertissement LNK4199,
// sans effet (les exemples qui ne touchent pas à ces DLL).
fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    if std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc") {
        for dll in [
            "mfplat.dll",
            "mfreadwrite.dll",
            "mf.dll",
            "dxgi.dll",
            "d3d11.dll",
            "bzs-absente.dll",
        ] {
            println!("cargo:rustc-link-arg-examples=/DELAYLOAD:{dll}");
        }
        println!("cargo:rustc-link-arg-examples=delayimp.lib");
    }
}
