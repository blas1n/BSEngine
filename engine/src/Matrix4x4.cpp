#include "Matrix4x4.h"
#include "Quaternion.h"
#include "MathFunctions.h"
#include <algorithm>

namespace BE
{
	namespace Math
	{
		const Matrix4x4 Matrix4x4::Zero
		{
			0.0f, 0.0f, 0.0f, 0.0f,
			0.0f, 0.0f, 0.0f, 0.0f,
			0.0f, 0.0f, 0.0f, 0.0f,
			0.0f, 0.0f, 0.0f, 0.0f
		};

		const Matrix4x4 Matrix4x4::One
		{
			1.0f, 1.0f, 1.0f, 1.0f,
			1.0f, 1.0f, 1.0f, 1.0f,
			1.0f, 1.0f, 1.0f, 1.0f,
			1.0f, 1.0f, 1.0f, 1.0f
		};

		const Matrix4x4 Matrix4x4::Identity
		{
			1.0f, 0.0f, 0.0f, 0.0f,
			0.0f, 1.0f, 0.0f, 0.0f,
			0.0f, 0.0f, 1.0f, 0.0f,
			0.0f, 0.0f, 0.0f, 1.0f
		};

		Matrix4x4 operator*(const Matrix4x4& lhs, const Matrix4x4& rhs) noexcept
		{
			auto transRhs = Matrix4x4::Transpose(rhs);
			return Matrix4x4
			{
				Vector4::Dot(lhs[0], transRhs[0]),
				Vector4::Dot(lhs[0], transRhs[1]),
				Vector4::Dot(lhs[0], transRhs[2]),
				Vector4::Dot(lhs[0], transRhs[3]),
				Vector4::Dot(lhs[1], transRhs[0]),
				Vector4::Dot(lhs[1], transRhs[1]),
				Vector4::Dot(lhs[1], transRhs[2]),
				Vector4::Dot(lhs[1], transRhs[3]),
				Vector4::Dot(lhs[2], transRhs[0]),
				Vector4::Dot(lhs[2], transRhs[1]),
				Vector4::Dot(lhs[2], transRhs[2]),
				Vector4::Dot(lhs[2], transRhs[3]),
				Vector4::Dot(lhs[3], transRhs[0]),
				Vector4::Dot(lhs[3], transRhs[1]),
				Vector4::Dot(lhs[3], transRhs[2]),
				Vector4::Dot(lhs[3], transRhs[3])
			};
		}

		Matrix4x4 Matrix4x4::FromQuaternion(const Quaternion& q) noexcept
		{
			return Matrix4x4
			{
				1.0f - 2.0f * q[1] * q[1] - 2.0f * q[2] * q[2],
				2.0f * q[0] * q[1] + 2.0f * q[3] * q[2],
				2.0f * q[0] * q[2] - 2.0f * q[3] * q[1],
				0.0f,

				2.0f * q[0] * q[1] - 2.0f * q[3] * q[2],
				1.0f - 2.0f * q[0] * q[0] - 2.0f * q[2] * q[2],
				2.0f * q[1] * q[2] + 2.0f * q[3] * q[0],
				0.0f,

				2.0f * q[0] * q[2] + 2.0f * q[3] * q[1],
				2.0f * q[1] * q[2] - 2.0f * q[3] * q[0],
				1.0f - 2.0f * q[0] * q[0] - 2.0f * q[1] * q[1],
				0.0f,

				0.0f, 0.0f, 0.0f, 1.0f
			};
		}

		Matrix4x4 Matrix4x4::FromRotationX(float theta) noexcept
		{
			return Matrix4x4
			{
				1.0f, 0.0f, 0.0f, 0.0f,
				0.0f, Math::Cos(theta), Math::Sin(theta), 0.0f,
				0.0f, -Math::Sin(theta), Math::Cos(theta), 0.0f,
				0.0f, 0.0f, 0.0f, 1.0f
			};
		}

		Matrix4x4 Matrix4x4::FromRotationY(float theta) noexcept
		{
			return Matrix4x4
			{
				Math::Cos(theta), 0.0f, -Math::Sin(theta), 0.0f,
				0.0f, 1.0f, 0.0f, 0.0f,
				Math::Sin(theta), 0.0f, Math::Cos(theta), 0.0f,
				0.0f, 0.0f, 0.0f, 1.0f
			};
		}

		Matrix4x4 Matrix4x4::FromRotationZ(float theta) noexcept
		{
			return Matrix4x4
			{
				Math::Cos(theta), Math::Sin(theta), 0.0f, 0.0f,
				-Math::Sin(theta), Math::Cos(theta), 0.0f, 0.0f,
				0.0f, 0.0f, 1.0f, 0.0f,
				0.0f, 0.0f, 0.0f, 1.0f
			};
		}

		inline float GetDeterminant(const Matrix4x4& mat) noexcept
		{
			return
				mat[0][0] * mat[1][1] * mat[2][2] * mat[3][3] -
				mat[0][0] * mat[1][1] * mat[2][3] * mat[3][2] -
				mat[0][0] * mat[1][2] * mat[2][1] * mat[3][3] +
				mat[0][0] * mat[1][2] * mat[2][3] * mat[3][1] +
				mat[0][0] * mat[1][3] * mat[2][1] * mat[3][2] -
				mat[0][0] * mat[1][3] * mat[2][2] * mat[3][1] -
				mat[0][1] * mat[1][0] * mat[2][2] * mat[3][3] +
				mat[0][1] * mat[1][0] * mat[2][3] * mat[3][2] +
				mat[0][1] * mat[1][2] * mat[2][1] * mat[3][3] -
				mat[0][1] * mat[1][2] * mat[2][3] * mat[3][0] -
				mat[0][1] * mat[1][3] * mat[2][0] * mat[3][2] +
				mat[0][1] * mat[1][3] * mat[2][2] * mat[3][0] +
				mat[0][2] * mat[1][0] * mat[2][1] * mat[3][3] -
				mat[0][2] * mat[1][0] * mat[2][3] * mat[3][1] -
				mat[0][2] * mat[1][1] * mat[2][0] * mat[3][3] +
				mat[0][2] * mat[1][1] * mat[2][3] * mat[3][0] +
				mat[0][2] * mat[1][3] * mat[2][0] * mat[3][1] -
				mat[0][2] * mat[1][3] * mat[2][1] * mat[3][0] -
				mat[0][3] * mat[1][0] * mat[2][1] * mat[3][2] +
				mat[0][3] * mat[1][0] * mat[2][2] * mat[3][1] +
				mat[0][3] * mat[1][1] * mat[2][0] * mat[3][2] -
				mat[0][3] * mat[1][1] * mat[2][2] * mat[3][0] -
				mat[0][3] * mat[1][2] * mat[2][0] * mat[3][1] +
				mat[0][3] * mat[1][2] * mat[2][1] * mat[3][0];
		}

		void Matrix4x4::Inverted() noexcept
		{
			const Matrix4x4 ret
			{
				// 00
				rows[1][1] * rows[2][2] * rows[3][3] +
				rows[1][2] * rows[2][3] * rows[3][1] +
				rows[1][3] * rows[2][1] * rows[3][2] -
				rows[1][1] * rows[2][3] * rows[3][2] -
				rows[1][2] * rows[2][1] * rows[3][3] -
				rows[1][3] * rows[2][2] * rows[3][1],

				// 01
				rows[0][1] * rows[2][3] * rows[3][2] +
				rows[0][2] * rows[2][1] * rows[3][3] +
				rows[0][3] * rows[2][2] * rows[3][1] -
				rows[0][1] * rows[2][2] * rows[3][3] -
				rows[0][2] * rows[2][3] * rows[3][1] -
				rows[0][3] * rows[2][1] * rows[3][2],

				// 02
				rows[0][1] * rows[1][2] * rows[3][3] +
				rows[0][2] * rows[1][3] * rows[3][1] +
				rows[0][3] * rows[1][1] * rows[3][2] -
				rows[0][1] * rows[1][3] * rows[3][2] -
				rows[0][2] * rows[1][1] * rows[3][3] -
				rows[0][3] * rows[1][2] * rows[3][1],

				// 03
				rows[0][1] * rows[1][3] * rows[2][2] +
				rows[0][2] * rows[1][1] * rows[2][3] +
				rows[0][3] * rows[1][2] * rows[2][1] -
				rows[0][1] * rows[1][2] * rows[2][3] -
				rows[0][2] * rows[1][3] * rows[2][1] -
				rows[0][3] * rows[1][1] * rows[2][2],

				// 10
				rows[1][0] * rows[2][3] * rows[3][2] +
				rows[1][2] * rows[2][0] * rows[3][3] +
				rows[1][3] * rows[2][2] * rows[3][0] -
				rows[1][0] * rows[2][2] * rows[3][3] -
				rows[1][2] * rows[2][3] * rows[3][0] -
				rows[1][3] * rows[2][0] * rows[3][2],

				// 11
				rows[0][0] * rows[2][2] * rows[3][3] +
				rows[0][2] * rows[2][3] * rows[3][0] +
				rows[0][3] * rows[2][0] * rows[3][2] -
				rows[0][0] * rows[2][3] * rows[3][2] -
				rows[0][2] * rows[2][0] * rows[3][3] -
				rows[0][3] * rows[2][2] * rows[3][0],

				// 12
				rows[0][0] * rows[1][3] * rows[3][2] +
				rows[0][2] * rows[1][0] * rows[3][3] +
				rows[0][3] * rows[1][2] * rows[3][0] -
				rows[0][0] * rows[1][2] * rows[3][3] -
				rows[0][2] * rows[1][3] * rows[3][0] -
				rows[0][3] * rows[1][0] * rows[3][2],

				// 13
				rows[0][0] * rows[1][1] * rows[2][3] +
				rows[0][1] * rows[1][3] * rows[2][0] +
				rows[0][3] * rows[1][0] * rows[2][1] -
				rows[0][0] * rows[1][3] * rows[2][2] -
				rows[0][2] * rows[1][0] * rows[2][3] -
				rows[0][3] * rows[1][2] * rows[2][0],

				// 20
				rows[1][0] * rows[2][1] * rows[3][3] +
				rows[1][1] * rows[2][3] * rows[3][0] +
				rows[1][3] * rows[2][0] * rows[3][1] -
				rows[1][0] * rows[2][3] * rows[3][1] -
				rows[1][1] * rows[2][0] * rows[3][3] -
				rows[1][3] * rows[2][1] * rows[3][0],

				// 21
				rows[0][0] * rows[2][3] * rows[3][1] +
				rows[0][1] * rows[2][0] * rows[3][3] +
				rows[0][3] * rows[2][1] * rows[3][0] -
				rows[0][0] * rows[2][1] * rows[3][3] -
				rows[0][1] * rows[2][3] * rows[3][0] -
				rows[0][3] * rows[2][0] * rows[3][1],

				// 22
				rows[0][0] * rows[1][1] * rows[3][3] +
				rows[0][1] * rows[1][3] * rows[3][0] +
				rows[0][3] * rows[1][0] * rows[3][1] -
				rows[0][0] * rows[1][3] * rows[3][1] -
				rows[0][1] * rows[1][0] * rows[3][3] -
				rows[0][3] * rows[1][1] * rows[3][0],

				// 23
				rows[0][0] * rows[1][1] * rows[2][3] +
				rows[0][1] * rows[1][3] * rows[2][0] +
				rows[0][3] * rows[1][0] * rows[2][1] -
				rows[0][0] * rows[1][3] * rows[2][1] -
				rows[0][1] * rows[1][0] * rows[2][3] -
				rows[0][3] * rows[1][1] * rows[2][0],

				// 30
				rows[1][0] * rows[2][2] * rows[3][1] +
				rows[1][1] * rows[2][0] * rows[3][2] +
				rows[1][2] * rows[2][1] * rows[3][0] -
				rows[1][0] * rows[2][1] * rows[3][2] -
				rows[1][1] * rows[2][2] * rows[3][0] -
				rows[1][2] * rows[2][0] * rows[3][1],

				// 31
				rows[0][0] * rows[2][1] * rows[3][2] +
				rows[0][1] * rows[2][2] * rows[3][0] +
				rows[0][2] * rows[2][0] * rows[3][1] -
				rows[0][0] * rows[2][2] * rows[3][1] -
				rows[0][1] * rows[2][0] * rows[3][2] -
				rows[0][2] * rows[2][1] * rows[3][0],

				// 32
				rows[0][0] * rows[1][2] * rows[3][1] +
				rows[0][1] * rows[1][0] * rows[3][2] +
				rows[0][2] * rows[1][1] * rows[3][0] -
				rows[0][0] * rows[1][1] * rows[3][2] -
				rows[0][1] * rows[1][2] * rows[3][0] -
				rows[0][2] * rows[1][0] * rows[3][1],

				// 33
				rows[0][0] * rows[1][1] * rows[2][2] +
				rows[0][1] * rows[1][2] * rows[2][0] +
				rows[0][2] * rows[1][0] * rows[2][1] -
				rows[0][0] * rows[1][2] * rows[2][1] -
				rows[0][1] * rows[1][0] * rows[2][2] -
				rows[0][2] * rows[1][1] * rows[2][0],
			};

			*this = ret / GetDeterminant(*this);
		}
	}
}