package graphics.kiln.blaze4d.core.natives;

import jdk.incubator.foreign.*;
import org.apache.logging.log4j.LogManager;
import org.apache.logging.log4j.Logger;
import org.apache.logging.log4j.message.StringFormatterMessageFactory;
import org.lwjgl.glfw.GLFW;
import org.lwjgl.system.APIUtil;

import java.lang.invoke.MethodHandle;
import java.lang.invoke.MethodHandles;
import java.lang.invoke.MethodType;
import java.nio.charset.StandardCharsets;
import java.util.Optional;

import static jdk.incubator.foreign.ValueLayout.*;

public class Natives {
    private static final Logger NATIVE_LOGGER = LogManager.getLogger("Blaze4DNative", new StringFormatterMessageFactory());

    public static final SymbolLookup lookup;
    public static final CLinker linker;

    public static final NativeMetadata nativeMetadata;

    public static final MethodHandle B4D_CREATE_GLFW_SURFACE_PROVIDER_HANDLE;
    public static final MethodHandle B4D_INIT_HANDLE;
    public static final MethodHandle B4D_DESTROY_HANDLE;
    public static final MethodHandle B4D_CREATE_SHADER_HANDLE;
    public static final MethodHandle B4D_DESTROY_SHADER_HANDLE;
    public static final MethodHandle B4D_START_FRAME_HANDLE;
    public static final MethodHandle B4D_END_FRAME_HANDLE;

    static {
        System.load(System.getProperty("b4d.native"));

        lookup = SymbolLookup.loaderLookup();
        linker = CLinker.systemCLinker();
        nativeMetadata = loadMetadata();
        initNativeLogger();
        preInitGlfw();

        B4D_CREATE_GLFW_SURFACE_PROVIDER_HANDLE = lookupFunction("b4d_create_glfw_surface_provider",
                FunctionDescriptor.of(ADDRESS, ADDRESS, ADDRESS, ADDRESS)
        );

        B4D_INIT_HANDLE = lookupFunction("b4d_init",
                FunctionDescriptor.of(ADDRESS, ADDRESS, JAVA_INT)
        );

        B4D_DESTROY_HANDLE = lookupFunction("b4d_destroy",
                FunctionDescriptor.ofVoid(ADDRESS)
        );

        B4D_CREATE_SHADER_HANDLE = lookupFunction("b4d_create_shader",
                FunctionDescriptor.of(JAVA_LONG, ADDRESS, ADDRESS, JAVA_LONG)
        );

        B4D_DESTROY_SHADER_HANDLE = lookupFunction("b4d_destroy_shader",
                FunctionDescriptor.ofVoid(ADDRESS, JAVA_LONG)
        );

        B4D_START_FRAME_HANDLE = lookupFunction("b4d_start_frame",
                FunctionDescriptor.of(ADDRESS, ADDRESS, JAVA_INT, JAVA_INT)
        );

        B4D_END_FRAME_HANDLE = lookupFunction("b4d_end_frame",
                FunctionDescriptor.ofVoid(ADDRESS)
        );
    }

    public static MemoryAddress b4dCreateGlfwSurfaceProvider(long glfwWindow) {
        MemoryAddress pfnGlfwGetRequiredInstanceExtensions = MemoryAddress.ofLong(APIUtil.apiGetFunctionAddress(GLFW.getLibrary(), "glfwGetRequiredInstanceExtensions"));
        MemoryAddress pfnGlfwCreateWindowSurface = MemoryAddress.ofLong(APIUtil.apiGetFunctionAddress(GLFW.getLibrary(), "glfwCreateWindowSurface"));
        try {
            return (MemoryAddress) B4D_CREATE_GLFW_SURFACE_PROVIDER_HANDLE.invoke(MemoryAddress.ofLong(glfwWindow), pfnGlfwGetRequiredInstanceExtensions, pfnGlfwCreateWindowSurface);
        } catch (Throwable e) {
            throw new RuntimeException("Failed to invoke b4d_create_glfw_surface_provider", e);
        }
    }

    public static MemoryAddress b4dInit(MemoryAddress surface, boolean enableValidation) {
        int enableValidationInt = enableValidation ? 1 : 0;
        try {
            return (MemoryAddress) B4D_INIT_HANDLE.invoke(surface, enableValidationInt);
        } catch (Throwable e) {
            throw new RuntimeException("Failed to invoke b4d_init", e);
        }
    }

    public static void b4dDestroy(MemoryAddress b4d) {
        try {
            B4D_DESTROY_HANDLE.invoke(b4d);
        } catch (Throwable e) {
            throw new RuntimeException("Failed to invoke b4d_destroy", e);
        }
    }

    public static long b4dCreateShader(MemoryAddress b4d, MemoryAddress vertexFormat, long usedUniforms) {
        try {
            return (long) B4D_CREATE_SHADER_HANDLE.invoke(b4d, vertexFormat, usedUniforms);
        } catch (Throwable e) {
            throw new RuntimeException("Failed to invoke b4d_create_shader", e);
        }
    }

    public static void b4dDestroyShader(MemoryAddress b4d, long shaderId) {
        try {
            B4D_DESTROY_SHADER_HANDLE.invoke(b4d, shaderId);
        } catch (Throwable e) {
            throw new RuntimeException("Failed to invoke b4d_destroy_shader", e);
        }
    }

    public static MemoryAddress b4dStartFrame(MemoryAddress b4d, int windowWidth, int windowHeight) {
        try {
            return (MemoryAddress) B4D_START_FRAME_HANDLE.invoke(b4d, windowWidth, windowHeight);
        } catch (Throwable e) {
            throw new RuntimeException("Failed to invoke b4d_start_frame", e);
        }
    }

    public static void b4dEndFrame(MemoryAddress frame) {
        try {
            B4D_END_FRAME_HANDLE.invoke(frame);
        } catch (Throwable e) {
            throw new RuntimeException("Failed to invoke b4d_end_frame", e);
        }
    }

    public record NativeMetadata(int sizeBytes) {
    }

    public static ValueLayout getSizeLayout() {
        // Only 64 bit is supported right now
        return JAVA_LONG;
    }

    private static MethodHandle lookupFunction(String name, FunctionDescriptor descriptor) {
        Optional<NativeSymbol> result = lookup.lookup(name);
        if (result.isPresent()) {
            return linker.downcallHandle(result.get(), descriptor);
        }
        throw new UnsatisfiedLinkError("Failed to find Blaze4D core function \"" + name + "\"");
    }

    private static NativeMetadata loadMetadata() {
        MethodHandle b4dGetNativeMetadataHandle = lookupFunction("b4d_get_native_metadata",
                FunctionDescriptor.of(ADDRESS)
        );

        MemoryLayout layout = MemoryLayout.structLayout(
                JAVA_INT.withName("size_bytes")
        );

        MemoryAddress address;
        try {
            address = (MemoryAddress) b4dGetNativeMetadataHandle.invoke();
        } catch (Throwable e) {
            throw new RuntimeException("Failed to invoke b4d_get_native_metadata", e);
        }

        MemorySegment segment = MemorySegment.ofAddress(address, layout.byteSize(), ResourceScope.globalScope());
        int sizeBytes = segment.get(JAVA_INT, layout.byteOffset(PathElement.groupElement("size_bytes")));

        if(sizeBytes != 8) {
            throw new RuntimeException("Blaze4D natives have 4byte size type. We do not support 32bit right now.");
        }

        return new NativeMetadata(sizeBytes);
    }

    private static void preInitGlfw() {
        MethodHandle b4dPreInitGlfwHandle = lookupFunction("b4d_pre_init_glfw",
                FunctionDescriptor.ofVoid(ADDRESS)
        );

        try {
            b4dPreInitGlfwHandle.invoke(MemoryAddress.ofLong(APIUtil.apiGetFunctionAddress(GLFW.getLibrary(), "glfwInitVulkanLoader")));
        } catch (Throwable e) {
            throw new RuntimeException("Failed to invoke b4d_pre_init_glfw", e);
        }
    }

    private static void initNativeLogger() {
        MethodHandle b4dInitExternalLogger = lookupFunction("b4d_init_external_logger",
                FunctionDescriptor.ofVoid(ADDRESS)
        );

        try {
            MethodHandle logFn = MethodHandles.lookup().findStatic(Natives.class, "nativeLogHandler",
                    MethodType.methodType(Void.TYPE, MemoryAddress.class, MemoryAddress.class, Integer.TYPE, Integer.TYPE, Integer.TYPE));
            NativeSymbol logFnNative = linker.upcallStub(
                    logFn,
                    FunctionDescriptor.ofVoid(ADDRESS, ADDRESS, JAVA_INT, JAVA_INT, JAVA_INT),
                    ResourceScope.globalScope()
            );
            b4dInitExternalLogger.invoke(logFnNative);
        } catch (Throwable e) {
            throw new RuntimeException("Failed to init b4d native logger", e);
        }
    }

    private static void nativeLogHandler(MemoryAddress targetPtr, MemoryAddress msgPtr, int targetLen, int msgLen, int level) {
        try (ResourceScope scope = ResourceScope.newConfinedScope()) {
            MemorySegment target = MemorySegment.ofAddress(targetPtr, targetLen, scope);
            MemorySegment message = MemorySegment.ofAddress(msgPtr, msgLen, scope);

            byte[] targetData = target.toArray(ValueLayout.JAVA_BYTE);
            byte[] messageData = message.toArray(ValueLayout.JAVA_BYTE);

            String targetString = new String(targetData, StandardCharsets.UTF_8);
            String messageString = new String(messageData, StandardCharsets.UTF_8);

            switch (level) {
                case 0 -> NATIVE_LOGGER.trace(messageString);
                case 1 -> NATIVE_LOGGER.debug(messageString);
                case 2 -> NATIVE_LOGGER.info(messageString);
                case 3 -> NATIVE_LOGGER.warn(messageString);
                case 4 -> NATIVE_LOGGER.error(messageString);
                default -> NATIVE_LOGGER.error("Received invalid log level from b4d native: " + level);
            }
        } catch (Throwable e) {
            NATIVE_LOGGER.error("Failed to log native message", e);
        }
    }

    public static void verifyInit() {
    }
}
