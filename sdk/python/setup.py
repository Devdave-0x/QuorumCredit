from setuptools import setup, find_packages

with open("README.md", "r", encoding="utf-8") as fh:
    long_description = fh.read()

setup(
    name="quorum-credit",
    version="1.0.0",
    author="QuorumCredit Team",
    author_email="team@quorumcredit.io",
    description="Python SDK for QuorumCredit - Decentralized microlending on Stellar Soroban",
    long_description=long_description,
    long_description_content_type="text/markdown",
    url="https://github.com/QuorumCredit/QuorumCredit",
    packages=find_packages(),
    classifiers=[
        "Programming Language :: Python :: 3",
        "Programming Language :: Python :: 3.8",
        "Programming Language :: Python :: 3.9",
        "Programming Language :: Python :: 3.10",
        "Programming Language :: Python :: 3.11",
        "License :: OSI Approved :: MIT License",
        "Operating System :: OS Independent",
        "Development Status :: 4 - Beta",
        "Intended Audience :: Developers",
        "Topic :: Software Development :: Libraries :: Python Modules",
    ],
    python_requires=">=3.8",
    install_requires=[
        "stellar-sdk>=9.0.0",
    ],
    extras_require={
        "dev": [
            "pytest>=7.0.0",
            "pytest-asyncio>=0.21.0",
            "black>=23.0.0",
            "flake8>=6.0.0",
            "mypy>=1.0.0",
        ],
    },
    project_urls={
        "Bug Reports": "https://github.com/QuorumCredit/QuorumCredit/issues",
        "Source": "https://github.com/QuorumCredit/QuorumCredit",
        "Documentation": "https://github.com/QuorumCredit/QuorumCredit/docs",
    },
)
