#include "Random.h"
#include "Vector2.h"
#include "Vector3.h"
#include "Vector4.h"

namespace BE
{
	namespace Math
	{
		std::mt19937 Random::generator;

		Vector2 Random::GetRange(const Vector2& min, const Vector2& max) noexcept
		{
			const auto r = Vector2{ GetFloat(), GetFloat() };
			return min + (max - min) * r;
		}

		Vector3 Random::GetRange(const Vector3& min, const Vector3& max) noexcept
		{
			const auto r = Vector3{ GetFloat(), GetFloat(), GetFloat() };
			return min + (max - min) * r;
		}

		Vector4 Random::GetRange(const Vector4& min, const Vector4& max) noexcept
		{
			const auto r = Vector4{ GetFloat(), GetFloat(), GetFloat(), GetFloat() };
			return min + (max - min) * r;
		}
	}
}