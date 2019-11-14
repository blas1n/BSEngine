#include "Vector4.h"
#include "MathFunctions.h"

const Vector4 Vector4::Zero{ 0.0f, 0.0f, 0.0f, 0.0f };
const Vector4 Vector4::One{ 1.0f, 1.0f, 1.0f, 1.0f };
const Vector4 Vector4::UnitX{ 1.0f, 0.0f, 0.0f, 0.0f };
const Vector4 Vector4::UnitY{ 0.0f, 1.0f, 0.0f, 0.0f };
const Vector4 Vector4::UnitZ{ 0.0f, 0.0f, 1.0f, 0.0f };
const Vector4 Vector4::UnitW{ 0.0f, 0.0f, 0.0f, 1.0f };

float Vector4::LengthSquared() const noexcept
{
	return Math::Pow(x) + Math::Pow(y) + Math::Pow(z) + Math::Pow(w);
}

float Vector4::Length() const noexcept
{
	return Math::Sqrt(LengthSquared());
}