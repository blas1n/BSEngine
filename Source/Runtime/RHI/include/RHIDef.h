#pragma once

#include <variant>
#include "BSMath/Color.h"
#include "Assertion.h"

struct DepthStencil
{
	DepthStencil(float inDepth, BSBase::uint32 inStencil)
		: depth(inDepth), stencil(inStencil) {}

	float depth;
	BSBase::uint32 stencil;
};

bool operator==(const DepthStencil& lhs, const DepthStencil& rhs)
{
	return (lhs.depth == rhs.depth) && (lhs.stencil == rhs.stencil);
}

bool operator!=(const DepthStencil& lhs, const DepthStencil& rhs)
{
	return !(lhs == rhs);
}

class RHI_API ClearValue final
{
public:
	ClearValue() noexcept : value() {}

	explicit ClearValue(BSMath::Color inColor)
		: value(std::move(inColor)) {}

	explicit ClearValue(float inDepth, uint32 inStencil = 0)
		: value(std::in_place_type<DepthStencil>, inDepth, inStencil) {}

	BSMath::Color GetColor() const noexcept
	{
		Assert(std::holds_alternative<BSMath::Color>(value));
		return std::get<BSMath::Color>(value);
	}

	DepthStencil GetDepthStencil() const noexcept
	{
		Assert(std::holds_alternative<DepthStencil>(value));
		return std::get<DepthStencil>(value);
	}

	friend bool operator==(const ClearValue& lhs, const ClearValue& rhs);

private:
	std::variant<BSMath::Color, DepthStencil> value;
};

bool operator==(const ClearValue& lhs, const ClearValue& rhs)
{
	return lhs.value == rhs.value;
}

bool operator!=(const ClearValue& lhs, const ClearValue& rhs)
{
	return !(lhs == rhs);
}

enum class PixelFormat : BSBase::uint8
{
    Unknown,
    A32B32G32R32F,
    B8G8R8A8,
    G8,
    G16,
    DXT1,
    DXT3,
    DXT5,
    UYVY,
    FloatRGB,
    FloatRGBA,
    DepthStencil,
    ShadowDepth,
    R32_FLOAT,
    G16R16,
    G16R16F,
    G16R16F_FILTER,
    G32R32F,
    A2B10G10R10,
    A16B16G16R16,
    D24,
    R16F,
    R16F_FILTER,
    BC5,
    V8U8,
    A1,
    FloatR11G11B10,
    A8,
    R32_UINT,
    R32_SINT,
    PVRTC2,
    PVRTC4,
    R16_UINT,
    R16_SINT,
    R16G16B16A16_UINT,
    R16G16B16A16_SINT,
    R5G6B5_UNORM,
    R8G8B8A8,
    A8R8G8B8,
    BC4,
    R8G8,
    ATC_RGB,
    ATC_RGBA_E,
    ATC_RGBA_I,
    X24_G8,
    ETC1,
    ETC2_RGB,
    ETC2_RGBA,
    R32G32B32A32_UINT,
    R16G16_UINT,
    ASTC_4x4,
    ASTC_6x6,
    ASTC_8x8,
    ASTC_10x10,
    ASTC_12x12,
    BC6H,
    BC7,
    R8_UINT,
    L8,
    XGXR8,
    R8G8B8A8_UINT,
    R8G8B8A8_SNORM,
    R16G16B16A16_UNORM,
    R16G16B16A16_SNORM,
    PLATFORM_HDR_0,
    PLATFORM_HDR_1,
    PLATFORM_HDR_2,
    NV12,
    R32G32_UINT,
    ETC2_R11_EAC,
    ETC2_RG11_EAC,
    R8,
    MAX
};
