#!/usr/bin/env python3

import requests
import argparse
import os
from typing import List
from datetime import datetime

class GitHubPRFinder:
    def __init__(self, token: str):
        self.owner = "maidsafe"
        self.repo = "autonomi"
        self.token = token
        self.api_url = f"https://api.github.com/repos/{self.owner}/{self.repo}/commits"

    def get_pr_for_commit(self, commit_sha: str) -> List[dict]:
        """
        Retrieves the list of pull requests that include the given commit SHA.
        
        Args:
            commit_sha (str): The commit hash to search for.
        
        Returns:
            List[dict]: A list of pull request data dictionaries.
        """
        headers = {
            'Accept': 'application/vnd.github.groot-preview+json',
            'Authorization': f'token {self.token}'
        }
        url = f"{self.api_url}/{commit_sha}/pulls"
        response = requests.get(url, headers=headers)
        
        if response.status_code == 200:
            return response.json()
        else:
            return []

def parse_arguments() -> argparse.Namespace:
    """
    Parses command-line arguments.
    
    Returns:
        argparse.Namespace: The parsed arguments.
    """
    parser = argparse.ArgumentParser(description="Find merged PRs for commit hashes listed in a file.")
    parser.add_argument('--path', required=True, help='Path to the file containing commit hashes, one per line.')
    parser.add_argument('--token', help='GitHub personal access token. Can also be set via GITHUB_PAT_SAFE_NETWORK_PR_LIST environment variable.')
    return parser.parse_args()

def read_commits_from_file(file_path: str) -> List[str]:
    """
    Reads commit hashes from a file, one per line.
    
    Args:
        file_path (str): The path to the file containing commit hashes.
    
    Returns:
        List[str]: A list of commit hashes.
    """
    try:
        with open(file_path, 'r') as file:
            commits = [line.strip() for line in file if line.strip()]
        return commits
    except FileNotFoundError:
        return []
    except Exception:
        return []

def format_date(iso_date_str: str) -> str:
    """
    Formats an ISO 8601 date string to 'YYYY-MM-DD'.
    
    Args:
        iso_date_str (str): The ISO 8601 date string.
    
    Returns:
        str: The formatted date string.
    """
    try:
        date_obj = datetime.strptime(iso_date_str, "%Y-%m-%dT%H:%M:%SZ")
        return date_obj.strftime("%Y-%m-%d")
    except ValueError:
        return iso_date_str.split('T')[0] if 'T' in iso_date_str else iso_date_str

def main():
    args = parse_arguments()
    token = args.token or os.getenv('GITHUB_PAT_SAFE_NETWORK_PR_LIST')
    if not token:
        print("GitHub token not provided. Use --token argument or set GITHUB_PAT_SAFE_NETWORK_PR_LIST environment variable.")
        return

    commits = read_commits_from_file(args.path)
    if not commits:
        print("No commit hashes to process.")
        return

    finder = GitHubPRFinder(token=token)

    pr_entries = []
    no_pr_entries = []

    for commit in commits:
        prs = finder.get_pr_for_commit(commit)
        if prs:
            pr_found = False
            for pr in prs:
                merged_at = pr.get('merged_at')
                if merged_at:
                    pr_found = True
                    formatted_date = format_date(merged_at)
                    pr_entry = {
                        'date': formatted_date,
                        'commit': commit,
                        'pr_number': pr['number'],
                        'pr_title': pr['title']
                    }
                    pr_entries.append(pr_entry)
            if not pr_found:
                no_pr_entries.append(f"No merged PR found for commit {commit}.")
        else:
            no_pr_entries.append(f"No merged PR found for commit {commit}.")

    pr_entries_sorted = sorted(pr_entries, key=lambda x: x['date'])

    for entry in pr_entries_sorted:
        print(f"{entry['date']} - {entry['commit']} #{entry['pr_number']}: {entry['pr_title']}")

    for entry in no_pr_entries:
        print(entry)

if __name__ == "__main__":
    main()
