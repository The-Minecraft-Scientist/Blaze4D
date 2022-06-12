package graphics.kiln.blaze4d.core;

import graphics.kiln.blaze4d.core.natives.Natives;
import graphics.kiln.blaze4d.core.natives.VertexFormatNative;
import jdk.incubator.foreign.MemoryAddress;

public class Blaze4DCore implements AutoCloseable {

    private final MemoryAddress handle;

    public Blaze4DCore(long glfwWindow) {
        boolean enableValidation = System.getProperty("b4d.enable_validation") != null;

        MemoryAddress surfaceProvider = Natives.b4dCreateGlfwSurfaceProvider(glfwWindow);
        this.handle = Natives.b4dInit(surfaceProvider, enableValidation);
    }

    public long createShader(VertexFormatNative vertexFormat, long usedUniforms) {
        return Natives.b4dCreateShader(this.handle, vertexFormat.getAddress(), usedUniforms);
    }

    public void destroyShader(long shaderId) {
        Natives.b4dDestroyShader(this.handle, shaderId);
    }

    public Frame startFrame(int windowWidth, int windowHeight) {
        MemoryAddress frame = Natives.b4dStartFrame(this.handle, windowWidth, windowHeight);
        if(frame.toRawLongValue() == 0L) {
            return null;
        } else {
            return new Frame(frame);
        }
    }

    @Override
    public void close() throws Exception {
        Natives.b4dDestroy(this.handle);
    }
}
