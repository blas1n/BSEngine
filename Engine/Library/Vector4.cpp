#include "Vector4.h"
#include "Vector2.h"
#include "Vector3.h"

namespace BE::Math
{
	Vector4::operator Vector2() const noexcept
	{
		return Vector2{ x(), y() };
	}

	Vector4::operator Vector3() const noexcept
	{
		return Vector3{ x(), y(), z() };
	}
}