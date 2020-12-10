#pragma once

#include "Resource.h"

namespace ArenaBoss
{
	using uint = unsigned int;

	enum class VertexLayout
	{
		Undefined = 0,
		PosNormTex = 1,
		PosNormSkinTex = 2
	};

	struct VertexArrayParam final
	{
		VertexLayout layout;

		const void* verts;
		uint numVerts;

		const uint* indices;
		uint numIndices;
	};

	class VertexArray final : public Resource
	{
		GENERATE_RESOURCE(VertexArray)

	public:
		VertexArray(const std::string& inName, const VertexArrayParam& param);

		VertexArray(const VertexArray&) = delete;
		VertexArray(VertexArray&&) = delete;

		VertexArray& operator=(const VertexArray&) = delete;
		VertexArray& operator=(VertexArray&&) = delete;

		~VertexArray();

		void Activate();

		inline VertexLayout GetLayout() const noexcept { return layout; }
		inline uint GetNumVerts() const noexcept { return numIndices; }
		inline uint GetNumIndices() const noexcept { return numIndices; }

	private:
		VertexLayout layout = VertexLayout::Undefined;

		uint vertexBuffer = 0u;
		uint numVerts = 0u;

		uint indexBuffer = 0u;
		uint numIndices = 0u;

		uint vertexArray = 0u;
	};
}