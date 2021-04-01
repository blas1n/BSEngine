cmake_minimum_required (VERSION 3.10)
project (${name} NONE)
include (ExternalProject)

ExternalProject_Add (${name}
	GIT_REPOSITORY		${repo}
	GIT_TAG				${tag}
	GIT_SHALLOW			TRUE
	GIT_PROGRESS		TRUE
	SOURCE_DIR			"${src_dir}"
	BINARY_DIR			"${build_dir}"
	CONFIGURE_COMMAND	""
	BUILD_COMMAND		""
	INSTALL_COMMAND		""
	TEST_COMMAND		""
)
