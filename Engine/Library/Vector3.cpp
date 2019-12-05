#include "Vector3.h"
#include "Vector2.h"
#include "Vector4.h"

namespace BE::Math
{
	explicit Vector3::operator Vector2() const noexcept
	{
		return Vector2{ x(), y() };
	}

	explicit Vector3::operator Vector4() const noexcept
	{
		return Vector4{ x(), y(), z(), 0.0f };
	}
}