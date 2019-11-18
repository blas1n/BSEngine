#include "MathFunctions.h"
#include "Vector2.h"
#include "Vector3.h"
#include "Vector4.h"

namespace BE::Math
{
	bool NearEqual(const Vector2& lhs, const Vector2& rhs, const float epsilon /*= MACHINE_EPSILON*/)
	{
		return (lhs - rhs).LengthSquared() <= Pow(epsilon);
	}

	bool NearEqual(const Vector3& lhs, const Vector3& rhs, const float epsilon /*= MACHINE_EPSILON*/)
	{
		return (lhs - rhs).LengthSquared() <= Pow(epsilon);
	}

	bool NearEqual(const Vector4& lhs, const Vector4& rhs, const float epsilon /*= MACHINE_EPSILON*/)
	{
		return (lhs - rhs).LengthSquared() <= Pow(epsilon);
	}

	Vector2 Lerp(const Vector2& a, const Vector2& b, const float delta)
	{
		return a + delta * (b - a);
	}

	Vector3 Lerp(const Vector3& a, const Vector3& b, const float delta)
	{
		return a + delta * (b - a);
	}

	Vector4 Lerp(const Vector4& a, const Vector4& b, const float delta)
	{
		return a + delta * (b - a);
	}
}