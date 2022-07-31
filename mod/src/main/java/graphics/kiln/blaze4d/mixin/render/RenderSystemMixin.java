package graphics.kiln.blaze4d.mixin.render;

import com.mojang.blaze3d.systems.RenderSystem;
import com.mojang.blaze3d.vertex.Tesselator;
import graphics.kiln.blaze4d.Blaze4D;
import org.apache.commons.compress.harmony.pack200.NewAttributeBands;
import org.lwjgl.glfw.GLFW;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Overwrite;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

@Mixin(value = RenderSystem.class, remap = false)
public class RenderSystemMixin {

    @Inject(method="depthMask", at=@At("HEAD"))
    private static void setDepthWrite(boolean enable, CallbackInfo ci) {
        Blaze4D.depthWriteEnable = enable;
    }
//
//    @Inject(method = "maxSupportedTextureSize", at = @At("HEAD"), cancellable = true)
//    private static void setMaxSupportedTextureSize(CallbackInfoReturnable<Integer> cir) {
//        cir.setReturnValue(1024 * 1000);
//    }
//
//    /**
//     * @author Blaze4D
//     * @reason Removal Of GL Specific Code
//     */
//    @Overwrite
//    public static void flipFrame(long window) {
//        RenderSystem.replayQueue();
//        Tesselator.getInstance().getBuilder().clear();
//        GLFW.glfwPollEvents();
//        GlobalRenderSystem.postDraw();
//    }
}
