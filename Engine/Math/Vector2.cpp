#include "Vector2.h"
#include "Vector3.h"
#include "Vector4.h"

namespace BE::Math
{
	Vector2::operator Vector3() const noexcept
	{
		return Vector3{ x(), y(), 0.0f };
	}

	Vector2::operator Vector4() const noexcept
	{
		return Vector4{ x(), y(), 0.0f, 0.0f };
	}
}