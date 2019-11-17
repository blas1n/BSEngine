#include "Vector3.h"
#include "Matrix4x4.h"
#include "MathFunctions.h"
#include "Quaternion.h"

namespace BE
{
	namespace Math
	{
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

		Vector3 Vector3::Transform(const Vector3& v, const Quaternion& q) noexcept
		{
			Vector3 qv{ q[0], q[1], q[2] };
			return v + 2.0f * Vector3::Cross(qv, Vector3::Cross(qv, v) + q[3] * v);
		}

		Vector3 Vector3::Transform(const Vector3& vec, const Matrix4x4& mat, float w /*= 1.0f*/) noexcept
		{
			return Vector3
			{
				vec.x * mat[0][0] + vec.y * mat[1][0] + vec.z * mat[2][0] + w * mat[3][0],
				vec.x * mat[0][1] + vec.y * mat[1][1] + vec.z * mat[2][1] + w * mat[3][1],
				vec.x * mat[0][2] + vec.y * mat[1][2] + vec.z * mat[2][2] + w * mat[3][2]
			};
		}

		Vector3 Vector3::TransformWithPerspDiv(const Vector3& vec, const Matrix4x4& mat, float w /*= 1.0f*/) noexcept
		{
			auto ret = Transform(vec, mat, w);
			const auto transformedW = vec.x * mat[0][3] + vec.y * mat[1][3] + vec.z * mat[2][3] + w * mat[3][3];

			if (!Math::NearEqual(transformedW, 0.0f))
				ret /= transformedW;

			return ret;
		}
	}
}