package dev.birb.wgpu.entity;

import net.minecraft.client.render.VertexConsumer;

public class DummyVertexConsumer implements VertexConsumer {
    @Override
    public VertexConsumer vertex(float x, float y, float z) {
        return this;
    }

    @Override
    public VertexConsumer color(int red, int green, int blue, int alpha) {
        return this;
    }

    @Override
    public VertexConsumer texture(float u, float v) {
        return this;
    }

    @Override
    public VertexConsumer overlay(int u, int v) {
        return this;
    }

    @Override
    public VertexConsumer light(int u, int v) {
        return this;
    }

    @Override
    public VertexConsumer normal(float x, float y, float z) {
        return this;
    }

}
