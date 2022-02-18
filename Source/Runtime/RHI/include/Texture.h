#pragma once

#include "BSMath/Vector.h"
#include "RHIDef.h"

using BSBase::uint32;

enum class TextureCreateFlags : uint32
{
    None = 0,
    RenderTargetable = 1 << 0,
    ResolveTargetable = 1 << 1,
    DepthStencilTargetable = 1 << 2,
    ShaderResource = 1 << 3,
    SRGB = 1 << 4,
    CPUWritable = 1 << 5,
    NoTiling = 1 << 6,
    VideoDecode = 1 << 7,
    Dynamic = 1 << 8,
    InputAttachmentRead = 1 << 9,
    Foveation = 1 << 10,
    Memoryless = 1 << 11,
    GenerateMipCapable = 1 << 12,
    FastVRAMPartialAlloc = 1 << 13,
    DisableSRVCreation = 1 << 14,
    DisableDCC = 1 << 15,
    UAV = 1 << 16,
    Presentable = 1 << 17,
    CPUReadback = 1 << 18,
    OfflineProcessed = 1 << 19,
    FastVRAM = 1 << 20,
    HideInVisualizeTexture = 1 << 21,
    Virtual = 1 << 22,
    TargetArraySlicesIndependently = 1 << 23,
    Shared = 1 << 24,
    NoFastClear = 1 << 25,
    DepthStencilResolveTarget = 1 << 26,
    Streamable = 1 << 27,
    NoFastClearFinalize = 1 << 28,
    AFRManual = 1 << 29,
    ReduceMemoryWithTilingMode = 1 << 30,
    Transient = 1 << 31,
}

class RHI_API RHITexture
{
    RHITexture(uint32 inNumMips, uint32 inNumSamples, iPixelFormat inFormat, TextureCreateFlags inFlags, const ClearValue& inClearValue)
        : clearValue(inClearValue)
        , numMips(inNumMips)
        , numSamples(inNumSamples)
        , flags(inFlags)
        , format(inFormat) {}

    virtual void* GetNativeResource() const noexcept { return nullptr; }
    virtual void* GetShaderResourceView() const noexcept { return nullptr; }

    virtual IntVector GetSize() const noexcept = 0;

    ClearValue GetClearValue() const noexcept { return clearValue; }

    uint32 GetNumMipmaps() const noexcept { return numMips; }
    uint32 GetNumSamples() const noexcept { return numSamples; }

    TextureCreateFlags GetFlags() const noexcept { return flags; }
    PixelFormat GetFormat() const noexcept { return format; }

private:
    ClearValue clearValue;

    uint32 numMips;
    uint32 numSamples;

    TextureCreateFlags flags;
    PixelFormat format;
};
