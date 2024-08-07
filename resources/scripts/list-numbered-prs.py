#!/usr/bin/env python

import os
import sys
from collections import defaultdict
from github import Github

def has_breaking_change(commits):
    for commit in commits:
        commit_message = commit.commit.message
        if '!' in commit_message.split('\n')[0] or 'BREAKING CHANGE' in commit_message:
            return True
    return False


def main(pr_numbers):
    token = os.getenv("GITHUB_PAT_SAFE_NETWORK_PR_LIST")
    if not token:
        raise Exception("The GITHUB_PAT_SAFE_NETWORK_PR_LIST environment variable must be set")

    g = Github(token)
    repo = g.get_repo("maidsafe/safe_network")

    filtered_pulls = []
    for pr_num in pr_numbers:
        print(f"Processing #{pr_num}...")
        pull = repo.get_pull(pr_num)
        if not pull.closed_at and not pull.merged_at:
            raise Exception(f"PR {pr_num} has not been closed yet")
        commits = pull.get_commits()
        breaking = has_breaking_change(commits)
        filtered_pulls.append({
            "number": pull.number,
            "title": pull.title,
            "author": pull.user.login,
            "closed_at": pull.closed_at,
            "breaking": breaking,
            "commits": commits
        })
    filtered_pulls.sort(key=lambda pr: pr["closed_at"])

    print("Flat list:")
    for pr in filtered_pulls:
        closed_date = pr["closed_at"].date()
        breaking_text = "[BREAKING]" if pr["breaking"] else ""
        print(f"{closed_date} #{pr['number']} -- {pr['title']} [@{pr['author']}] {breaking_text}")
    print("Flat list markdown:")
    for pr in filtered_pulls:
        pr_number = pr["number"]
        closed_date = pr["closed_at"].date()
        breaking_text = "[BREAKING]" if pr["breaking"] else ""
        print(f"{closed_date} [#{pr_number}](https://github.com/maidsafe/safe_network/pull/{pr_number}) -- {pr['title']} [@{pr['author']}] {breaking_text}")

    print()
    grouped_pulls = defaultdict(list)
    for pr in filtered_pulls:
        grouped_pulls[pr["author"]].append(pr)

    print("Grouped by author:")
    for author, prs in grouped_pulls.items():
        print(f"@{author}")
        for pr in prs:
            closed_date = pr["closed_at"].date()
            breaking_text = "[BREAKING]" if pr["breaking"] else ""
            print(f"  {closed_date} #{pr['number']} -- {pr['title']} {breaking_text}")
        print()

    print("Grouped by author with commits:")
    for author, prs in grouped_pulls.items():
        print(f"@{author}")
        for pr in prs:
            closed_date = pr["closed_at"].date()
            breaking_text = "[BREAKING]" if pr["breaking"] else ""
            print(f"  {closed_date} #{pr['number']} -- {pr['title']} {breaking_text}")
            for commit in pr["commits"]:
                print(f"    {commit.commit.message.split('\n')[0]}")
        print()

    print("Grouped by author markdown:")
    for author, prs in grouped_pulls.items():
        print(f"@{author}")
        for pr in prs:
            pr_number = pr["number"]
            closed_date = pr["closed_at"].date()
            breaking_text = "[BREAKING]" if pr["breaking"] else ""
            print(f"  {closed_date} [#{pr_number}](https://github.com/maidsafe/safe_network/pull/{pr_number}) -- {pr['title']} {breaking_text}")
        print()

def read_pr_numbers(file_path):
    with open(file_path, 'r') as file:
        return [int(line.strip()) for line in file]

if __name__ == "__main__":
    if len(sys.argv) != 2:
        print("Usage: python script.py <file_path>")
        sys.exit(1)
    
    file_path = sys.argv[1]
    main(read_pr_numbers(file_path))
