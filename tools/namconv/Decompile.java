// Ghidra headless script: decompile functions at the given offsets (relative to the image base)
// and write each to <outdir>/<name>.c.
//   analyzeHeadless <proj dir> <proj> -import imagelib_arm64 -postScript Decompile.java <outdir> name=0xOFF ...
//@category namconv
import ghidra.app.decompiler.DecompInterface;
import ghidra.app.decompiler.DecompileOptions;
import ghidra.app.decompiler.DecompileResults;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.listing.Function;
import java.io.File;
import java.io.PrintWriter;

public class Decompile extends GhidraScript {
    @Override
    public void run() throws Exception {
        String[] args = getScriptArgs();
        File outdir = new File(args[0]);
        outdir.mkdirs();
        DecompInterface d = new DecompInterface();
        DecompileOptions opts = new DecompileOptions();
        opts.setMaxPayloadMBytes(256);
        opts.setMaxInstructions(200000);
        d.setOptions(opts);
        d.openProgram(currentProgram);
        for (int i = 1; i < args.length; i++) {
            String[] kv = args[i].split("=");
            Address a = currentProgram.getImageBase().add(Long.decode(kv[1]));
            Function f = getFunctionAt(a);
            if (f == null) f = createFunction(a, null);
            if (f == null) { println("no function at " + a); continue; }
            DecompileResults r = d.decompileFunction(f, 300, monitor);
            try (PrintWriter w = new PrintWriter(new File(outdir, kv[0] + ".c"))) {
                w.println("// " + f.getName(true) + " @ " + a);
                w.println(r.decompileCompleted() ? r.getDecompiledFunction().getC() : ("// failed: " + r.getErrorMessage()));
            }
            println("decompiled " + kv[0] + " (" + f.getName(true) + ")");
        }
    }
}
