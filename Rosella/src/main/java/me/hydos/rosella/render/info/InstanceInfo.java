package me.hydos.rosella.render.info;

import me.hydos.rosella.Rosella;
import me.hydos.rosella.device.VulkanDevice;
import me.hydos.rosella.memory.Memory;
import me.hydos.rosella.memory.MemoryCloseable;
import me.hydos.rosella.render.material.Material;
import me.hydos.rosella.render.shader.ubo.Ubo;
import org.jetbrains.annotations.NotNull;

/**
 * Info such as the {@link Material} and {@link Ubo} for rendering objects
 */
public record InstanceInfo(Ubo ubo,
                           Material material) implements MemoryCloseable {

    @Override
    public void free(VulkanDevice device, Memory memory) {
        ubo.free(device, memory);
        material.getShader().getDescriptorManager().freeDescriptorSet(ubo.getDescriptors());
    }

    /**
     * Called when Command Buffers need to be refreshed. all {@link me.hydos.rosella.render.descriptorsets.DescriptorSet}'s will need to be recreated
     *
     * @param rosella the Rosella
     */
    public void rebuild(@NotNull Rosella rosella) {
        material.getShader().getDescriptorManager().freeDescriptorSet(ubo.getDescriptors());
        ubo.free(rosella.common.device, rosella.common.memory);

        if (ubo.getUniformBuffers().size() == 0) {
            ubo.create(rosella.renderer.swapchain);
        }

        material.getShader().getDescriptorManager().createNewDescriptor(material.textures, ubo);
    }
}
