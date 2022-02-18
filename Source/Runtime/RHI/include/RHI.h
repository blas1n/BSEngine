#pragma once

#include "Core.h"

class RHIMesh;
class RHIShader;
class RHITexture;

class RHI_API RHI
{
public:
	virtual ~RHI() = default;

	virtual RHIMesh* CreateMesh() { return nullptr; }
	virtual RHIMesh* CreateShader() { return nullptr; }
	virtual RHIMesh* CreateTexture() { return nullptr; }
};
