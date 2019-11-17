#include "Quaternion.h"
#include "MathFunctions.h"
#include "Vector3.h"

namespace BE
{
	namespace Math
	{
		Quaternion::Quaternion(const Vector3& axis, const float angle) noexcept
			: vec()
		{
			const auto scalar = Math::Sin(angle * 0.5f);
			vec.x = axis.x * scalar;
			vec.y = axis.y * scalar;
			vec.z = axis.z * scalar;
			vec.w = Math::Cos(angle * 0.5f);
		}

		Quaternion::Quaternion(const Vector3& euler) noexcept
			: vec()
		{
			const auto halfX = euler.x * 0.5f;
			const auto sinX = Math::Sin(halfX);
			const auto cosX = Math::Cos(halfX);

			const auto halfY = euler.y * 0.5f;
			const auto sinY = Math::Sin(halfY);
			const auto cosY = Math::Cos(halfY);

			const auto halfZ = euler.z * 0.5f;
			const auto sinZ = Math::Sin(halfZ);
			const auto cosZ = Math::Cos(halfZ);

			const auto cosYcosZ = cosY * cosZ;
			const auto sinYcosZ = sinY * cosZ;
			const auto cosYsinZ = cosY * sinZ;
			const auto sinYsinZ = sinY * sinZ;

			vec.x = sinX * cosYcosZ - cosX * sinYsinZ;
			vec.y = cosX * sinYcosZ + sinX * cosYsinZ;
			vec.z = cosX * cosYsinZ - sinX * sinYcosZ;
			vec.w = cosX * cosYcosZ + sinX * sinYsinZ;

			vec.Normalized();
		}

		Vector3 Quaternion::ToEuler() const noexcept
		{
			Vector3 ret;

			const auto sinRcosP = 2.0f * (vec.w * vec.x + vec.y * vec.z);
			const auto cosRcosP = 1.0f - (Math::Pow(vec.x) + Math::Pow(vec.y)) * 2.0f;
			ret.x = Math::Atan2(sinRcosP, cosRcosP);

			const auto sinP = (vec.w * vec.y - vec.z * vec.x) * 2.0f;
			if (Math::Abs(sinP) >= 1.0f)
				ret.y = Math::CopySign(Math::PI * 0.5f, sinP);
			else
				ret.y = Math::Asin(sinP);

			const auto sinYcosP = (vec.w * vec.z + vec.x * vec.y) * 2.0f;
			const auto cosYCosP = 1.0f - (Math::Pow(vec.y) + Math::Pow(vec.z)) * 2.0f;
			ret.z = Math::Atan2(sinYcosP, cosYCosP);

			return ret;
		}

		Quaternion Quaternion::Lerp(const Quaternion& a, const Quaternion& b, const float delta) noexcept
		{
			const auto ret = Math::Lerp(a.vec, b.vec, delta);
			const auto len =
				Math::Sqrt(Math::Pow(ret.x) + Math::Pow(ret.y) + Math::Pow(ret.z) + Math::Pow(ret.w));

			return Quaternion{ ret / len };
		}

		Quaternion Quaternion::Slerp(const Quaternion& a, const Quaternion& b, float delta) noexcept
		{
			const auto dotAB = Quaternion::Dot(a, b);
			const auto invert = dotAB > 0.0f ? 1.0f : -1.0f;
			const auto cosineTheta = dotAB * invert;

			if (1 - cosineTheta < Math::MACHINE_EPSILON)
				return Quaternion{ a.vec * (1.0f - delta) + b.vec * (delta * invert) };

			const auto theta = Math::Acos(cosineTheta);
			const auto sineTheta = Math::Sin(theta);

			const auto coeff1 = Math::Sin((1.0f - delta) * theta) / sineTheta;
			const auto coeff2 = Math::Sin(delta * theta) / sineTheta * invert;
			return Quaternion{ a.vec * coeff1 + b.vec * coeff2 };
		}

		Quaternion operator*(const Quaternion& lhs, const Quaternion& rhs) noexcept
		{
			Quaternion ret;

			const Vector3 lhsV{ lhs[0], lhs[1], lhs[2] };
			const Vector3 rhsV{ rhs[0], rhs[1], rhs[2] };
			const Vector3 v = lhsV * rhs[3] + rhsV * lhs[3] + Vector3::Cross(lhsV, rhsV);

			return Quaternion
			{
				v.x, v.y, v.z,
				lhs[3] * rhs[3] - Vector3::Dot(lhsV, rhsV)
			};
		}
	}
}