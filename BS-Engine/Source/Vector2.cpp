#include "Vector2.h"
#include "MathFunctions.h"

const Vector2 Vector2::Zero{ 0.0f, 0.0f };
const Vector2 Vector2::One{ 1.0f, 1.0f };
const Vector2 Vector2::UnitX{ 1.0f, 0.0f };
const Vector2 Vector2::UnitY{ 0.0f, 1.0f };

float Vector2::LengthSquared() const noexcept
{
	return Math::Pow(x) + Math::Pow(y);
}

float Vector2::Length() const noexcept
{
	return Math::Sqrt(LengthSquared());
}