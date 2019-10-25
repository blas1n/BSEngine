#include "Vector3.h"
#include "MathFunctions.h"

const Vector3 Vector3::Zero{ 0.0f, 0.0f, 0.0f };
const Vector3 Vector3::One{ 1.0f, 1.0f, 1.0f };
const Vector3 Vector3::UnitX{ 1.0f, 0.0f, 0.0f };
const Vector3 Vector3::UnitY{ 0.0f, 1.0f, 0.0f };
const Vector3 Vector3::UnitZ{ 0.0f, 0.0f, 1.0f };

float Vector3::LengthSquared() const noexcept
{
	return Math::Pow(x) + Math::Pow(y) + Math::Pow(z);
}

float Vector3::Length() const noexcept
{
	return Math::Sqrt(LengthSquared());
}