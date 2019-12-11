#include "Quaternion.h"
#include "Matrix4x4.h"
#include "Rotator.h"
#include "Vector3.h"
#include "Vector4.h"

namespace BE::Math
{
	Quaternion::Quaternion(float x, float y, float z, float w) noexcept
		: quat{ w, x, y, z } {}

	Quaternion::Quaternion(const float elems[4]) noexcept
		: quat{ elems } {}

	Quaternion::Quaternion(const Vector4& v) noexcept
		: quat{ v.w(), v.x(), v.y(), v.z() } {}

	Quaternion::Quaternion(const Vector3& axis, float angle) noexcept
		: quat{ Eigen::AngleAxisf{angle, Eigen::Vector3f{ axis.x(), axis.y(), axis.z() } } } {}

	Quaternion Quaternion::Lerp(const Quaternion& a, const Quaternion& b, const float f) {
		Quaternion ret
		{
			Math::Lerp(a.x(), b.x(), f),
			Math::Lerp(a.y(), b.y(), f),
			Math::Lerp(a.z(), b.z(), f),
			Math::Lerp(a.w(), b.w(), f)
		};

		ret.Normalized();
		return ret;
	}

	Matrix4x4 Quaternion::ToMatrix() const noexcept
	{
		Eigen::Matrix4f mat = Eigen::Matrix4f::Identity();
		mat.block(0, 0, 3, 3) = quat.toRotationMatrix();
		return Matrix4x4{ mat.data() };
	}

	Rotator Quaternion::ToRotator() const noexcept
	{
		Eigen::Vector3f vec = quat.toRotationMatrix().eulerAngles(0, 1, 2);
		return Rotator{ vec.data() };
	}
}