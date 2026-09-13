// Static analysis only; never opens a hardware transport.
// @category IO.Firmware
import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.symbol.SourceType;
import java.math.BigInteger;

public class SeedIoFirmware extends GhidraScript {
    public void run() throws Exception {
        if (!"8f5ef507771c6795258eb7521cfc1b46269a5b301548767ae87a3f1a83a50d45".equalsIgnoreCase(currentProgram.getExecutableSHA256()))
            throw new IllegalArgumentException("Only the reviewed IO White 1.17 HEX is supported");
        // Analysis regions, not a claim about installed memory capacity.
        if (getMemoryBlock(toAddr(0x20000000L)) == null)
            createMemoryBlock("RAM_analysis", toAddr(0x20000000L), null, 0x8000, false);
        if (getMemoryBlock(toAddr(0x40000000L)) == null)
            createMemoryBlock("MMIO_analysis", toAddr(0x40000000L), null, 0x90000, false);
        if (getMemoryBlock(toAddr(0x00601000L)) == null)
            createMemoryBlock("FMC_analysis", toAddr(0x00601000L), null, 0x1000, false);
        var context = currentProgram.getProgramContext();
        var tmode = currentProgram.getRegister("TMode");
        if (getInstructionAt(toAddr(0x204)) == null)
            context.setValue(tmode, toAddr(0), toAddr(0x16263), BigInteger.ONE);
        for (int offset = 4; offset < 0x1c0; offset += 4) {
            long target = Integer.toUnsignedLong(getInt(toAddr(offset)));
            if ((target & 1) == 1 && target < 0x16264) {
                Address entry = toAddr(target & ~1L);
                disassemble(entry);
                if (getFunctionAt(entry) == null) createFunction(entry, null);
            }
        }
        long[] seeds = {0x1c0,0x204,0xbf0,0xc3c,0x3630,0x3738,0x3aa8,0x51ae,0x54c4,0x6c90,0xd7c4,0xe540,0xeb60,0xeea0,0xf204,0xf4d8,0xf8d0,0x13028,0x1376c};
        for (long seed : seeds) {
            if (getInstructionAt(toAddr(seed)) == null)
                context.setValue(tmode, toAddr(seed), toAddr(seed), BigInteger.ONE);
            disassemble(toAddr(seed));
            if (getFunctionAt(toAddr(seed)) == null) createFunction(toAddr(seed), null);
        }
        long[] labels = {0x2000036cL,0x2000036dL,0x20005514L,0x20005592L,0x20005610L,0x20000108L};
        String[] names = {"calibration_mode", "simulation_mode", "rgb_plane0", "rgb_plane1", "rgb_plane2", "panel_depth_value"};
        for (int i=0;i<labels.length;i++)
            createLabel(toAddr(labels[i]), names[i], true, SourceType.USER_DEFINED);
    }
}
