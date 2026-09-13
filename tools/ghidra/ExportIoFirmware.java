// @category IO.Firmware
import ghidra.app.script.GhidraScript;
import ghidra.app.decompiler.DecompInterface;
import ghidra.program.model.listing.Function;
import java.nio.file.*;
import java.nio.charset.StandardCharsets;
import java.util.*;
import com.google.gson.GsonBuilder;

public class ExportIoFirmware extends GhidraScript {
    public void run() throws Exception {
        Path output = Path.of(getScriptArgs()[0]);
        Files.createDirectories(output.resolve("functions"));
        var decompiler = new DecompInterface();
        decompiler.openProgram(currentProgram);
        var index = new ArrayList<Map<String,Object>>();
        var functions = currentProgram.getFunctionManager().getFunctions(true);
        while (functions.hasNext() && !monitor.isCancelled()) {
            Function function = functions.next();
            var entry = new LinkedHashMap<String,Object>();
            String address = function.getEntryPoint().toString();
            entry.put("address", address);
            entry.put("name", function.getName());
            entry.put("bytes", function.getBody().getNumAddresses());
            var result = decompiler.decompileFunction(function, 30, monitor);
            entry.put("decompiled", result.decompileCompleted());
            if (result.decompileCompleted())
                Files.writeString(output.resolve("functions/"+address+".c"), result.getDecompiledFunction().getC(), StandardCharsets.UTF_8);
            else entry.put("error", result.getErrorMessage());
            var callers = new ArrayList<String>();
            for (var caller : function.getCallingFunctions(monitor)) callers.add(caller.getEntryPoint().toString());
            Collections.sort(callers);
            entry.put("callers", callers);
            var callees = new ArrayList<String>();
            for (var callee : function.getCalledFunctions(monitor)) callees.add(callee.getEntryPoint().toString());
            Collections.sort(callees);
            entry.put("callees", callees);
            index.add(entry);
        }
        decompiler.dispose();
        Files.writeString(output.resolve("functions.json"), new GsonBuilder().setPrettyPrinting().create().toJson(index), StandardCharsets.UTF_8);
        try(var writer = Files.newBufferedWriter(output.resolve("listing.asm"), StandardCharsets.UTF_8)) {
            var instructions = currentProgram.getListing().getInstructions(true);
            while(instructions.hasNext()) {
                var ins = instructions.next();
                writer.write(ins.getAddress()+"  "+ins+"\n");
            }
        }
        println("Exported "+index.size()+" function candidates; pseudocode is not recovered source.");
    }
}
