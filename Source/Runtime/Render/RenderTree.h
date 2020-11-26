#pragma once

#include <functional>
#include <vector>

namespace ArenaBoss
{
	class Shader;
	class MeshComponent;
	
	class RenderTree final
	{
	public:
		struct RenderNode
		{
			RenderNode(Shader* inShader)
				: shader(inShader), components() {}

			Shader* shader;
			std::vector<MeshComponent*> components;
		};

	public:
		void Draw(std::function<void(Shader&)> fn);

		void RegisterShader(Shader* shader);
		void UnregisterShader(Shader* shader);

		void RegisterComponent(MeshComponent* component);
		void UnregisterComponent(MeshComponent* component);

	private:
		std::vector<MeshComponent*>& GetComponents(Shader* shader);

	private:
		std::vector<RenderNode> nodes;
	};
}