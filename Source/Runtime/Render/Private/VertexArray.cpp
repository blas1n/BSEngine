#include "VertexArray.h"
#include <GL/glew.h>

namespace ArenaBoss
{
	namespace
	{
		ptrdiff_t GetVertexSize(VertexLayout layout)
		{
			switch (layout)
			{
			case VertexLayout::PosNormTex:
				return 8 * sizeof(float);

			case VertexLayout::PosNormSkinTex:
				return 8 * sizeof(float) + 8 * sizeof(char);
			}

			throw std::exception{ "Undefined layout" };
		}

		void EnablePosNormTex()
		{
			constexpr GLsizei size = 8 * sizeof(float);

			glEnableVertexAttribArray(0);
			glVertexAttribPointer(0, 3, GL_FLOAT, GL_FALSE, size, 0);

			glEnableVertexAttribArray(1);
			glVertexAttribPointer(1, 3, GL_FLOAT, GL_FALSE, size,
				reinterpret_cast<void*>(3 * sizeof(float)));

			glEnableVertexAttribArray(2);
			glVertexAttribPointer(2, 2, GL_FLOAT, GL_FALSE, size,
				reinterpret_cast<void*>(6 * sizeof(float)));
		}

		void EnablePosNormSkinTex()
		{
			constexpr GLsizei size = 8 * sizeof(float) + 8 * sizeof(char);

			glEnableVertexAttribArray(0);
			glVertexAttribPointer(0, 3, GL_FLOAT, GL_FALSE, size, 0);

			glEnableVertexAttribArray(1);
			glVertexAttribPointer(1, 3, GL_FLOAT, GL_FALSE, size,
				reinterpret_cast<void*>(3 * sizeof(float)));

			glEnableVertexAttribArray(2);
			glVertexAttribIPointer(2, 4, GL_UNSIGNED_BYTE, size,
				reinterpret_cast<void*>(6 * sizeof(float)));

			glEnableVertexAttribArray(3);
			glVertexAttribPointer(3, 4, GL_UNSIGNED_BYTE, GL_TRUE, size,
				reinterpret_cast<void*>(6 * sizeof(float) + 4 * sizeof(char)));

			glEnableVertexAttribArray(4);
			glVertexAttribPointer(4, 2, GL_FLOAT, GL_FALSE, size,
				reinterpret_cast<void*>(6 * sizeof(float) + 8 * sizeof(char)));
		}
	}

	VertexArray::VertexArray(const std::string& inName, const VertexArrayParam& param)
		: Resource(inName)
	{
		layout = param.layout;
		numVerts = param.numVerts;
		numIndices = param.numIndices;

		glGenVertexArrays(1, &vertexArray);
		glBindVertexArray(vertexArray);

		glGenBuffers(1, &vertexBuffer);
		glBindBuffer(GL_ARRAY_BUFFER, vertexBuffer);
		glBufferData(GL_ARRAY_BUFFER, numVerts * GetVertexSize(layout), param.verts, GL_STATIC_DRAW);

		glGenBuffers(1, &indexBuffer);
		glBindBuffer(GL_ELEMENT_ARRAY_BUFFER, indexBuffer);
		glBufferData(GL_ELEMENT_ARRAY_BUFFER, numIndices * sizeof(uint), param.indices, GL_STATIC_DRAW);

		using Fn = void(*)();
		constexpr static Fn enableFunctions[]{ EnablePosNormTex, EnablePosNormSkinTex };

		enableFunctions[static_cast<int32_t>(layout)]();
	}

	VertexArray::~VertexArray()
	{
		if (layout != VertexLayout::Undefined)
		{
			glDeleteBuffers(1, &indexBuffer);
			glDeleteBuffers(1, &vertexBuffer);
			glDeleteVertexArrays(1, &vertexArray);
		}
	}

	void VertexArray::Activate()
	{
		glBindVertexArray(vertexArray);
	}
}