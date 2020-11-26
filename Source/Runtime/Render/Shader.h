#pragma once

#include "GL/glew.h"
#include "Resource.h"
#include "Vector3.h"

namespace ArenaBoss
{
	namespace Math
	{
		class Matrix4x4;
	}

	class Shader final : public Resource
	{
		GENERATE_RESOURCE(Shader)

	public:
		Shader(const std::string& name,
			const std::string& vertName,
			const std::string& fragName);

		Shader(const Shader&) = delete;
		Shader(Shader&&) = delete;

		Shader& operator=(const Shader&) = delete;
		Shader& operator=(Shader&&) = delete;

		~Shader() override;

		void Activate();

		void SetUniformValue(const std::string& name, Math::Matrix4x4* matrices, uint32_t count);
		void SetUniformValue(const std::string& name, const Math::Matrix4x4& value);
		void SetUniformValue(const std::string& name, const Math::Vector3& value);
		void SetUniformValue(const std::string& name, float value);
		void SetUniformValue(const std::string& name, bool value);
		void SetUniformValue(const std::string& name, int value);

	private:
		bool CompileShader(const std::string& fileName, GLenum shaderType, GLuint& outShader);
		bool IsCompiled(GLuint shader);
		bool IsValidProgram();

	private:
		GLuint vertexShader = 0u;
		GLuint fragShader = 0u;
		GLuint shaderProgram = 0u;
	};
}