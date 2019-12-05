#include "Rotator.h"
#include <Eigen/Geometry>
#include "MathFunctions.h"
#include "Matrix4x4.h"
#include "Quaternion.h"

namespace BE::Math
{
	Rotator& Rotator::operator+=(const Rotator& other) noexcept
	{
		euler += other.euler;
		
		for (int i = 0; i < 3; ++i)
			euler[i] = Fmod(euler[i], 360.0f);

		return *this;
	}

	Rotator& Rotator::operator-=(const Rotator& other) noexcept
	{
		euler -= other.euler;

		for (int i = 0; i < 3; ++i)
			while (euler[i] < 0.0f)
				euler[i] += 360.0f;

		return *this;
	}

	Rotator& Rotator::operator*=(const Vector3& scale) noexcept
	{
		euler *= scale;

		for (int i = 0; i < 3; ++i)
			euler[i] = Fmod(euler[i], 360.0f);

		return *this;
	}

	Rotator& Rotator::operator/=(const Vector3& scale) noexcept
	{
		euler /= scale;
		return *this;
	}

	Rotator& Rotator::operator*=(const float scale) noexcept
	{
		euler *= scale;

		for (int i = 0; i < 3; ++i)
			euler[i] = Fmod(euler[i], 360.0f);

		return *this;
	}

	Rotator& Rotator::operator/=(const float scale) noexcept
	{
		euler /= scale;
		return *this;
	}

	Matrix4x4 Rotator::ToMatrix() const noexcept
	{
		const auto quat = GetQuaternion();
		Eigen::Matrix4f mat;
		mat.block(0, 0, 3, 3) = quat.toRotationMatrix();
		return Matrix4x4{ mat.data() };
	}

	Quaternion Rotator::ToQuaternion() const noexcept
	{
		const auto quat = GetQuaternion();
		return Quaternion{ quat.x(), quat.y(), quat.z(), quat.w() };
	}

	Eigen::Quaternionf Rotator::GetQuaternion() const noexcept
	{
		Eigen::Quaternionf quat
		{
			Eigen::AngleAxisf{ ToRadians(roll()), Eigen::Vector3f::UnitX() } *
			Eigen::AngleAxisf{ ToRadians(pitch()), Eigen::Vector3f::UnitY() } *
			Eigen::AngleAxisf{ ToRadians(yaw()), Eigen::Vector3f::UnitZ() }
		};

		quat.normalize();
		return quat;
	}
}