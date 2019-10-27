#include "Vector2.h"
#include "Matrix3x3.h"
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

Vector2 Vector2::Transform(const Vector2& vec, const Matrix3x3& mat, float w = 1.0f) noexcept
{
	return Vector2
	{
		vec.x * mat[0][0] + vec.y * mat[1][0] + w * mat[2][0],
		vec.x * mat[0][1] + vec.y * mat[1][1] + w * mat[2][1]
	};
}