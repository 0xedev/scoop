use anchor_lang::prelude::*;
use anchor_spl::token::{self, Token, TokenAccount, Transfer};
use anchor_lang::solana_program::system_instruction;

declare_id!("");


#[program]
pub mod noice_solana {
    use super::*;

    // Initialize a user profile
    pub fn initialize_user(ctx: Context<InitializeUser>) -> Result<()> {
        let user_profile = &mut ctx.accounts.user_profile;
        user_profile.owner = ctx.accounts.user.key();
        user_profile.interaction_count = 0;
        msg!("Initialized user profile for: {}", user_profile.owner);
        Ok(())
    }

    // Tip with any SPL token
    pub fn tip(
        ctx: Context<Tip>,
        amount: u64,
        action: String,
        _token_mint: Pubkey, // Passed for validation
    ) -> Result<()> {
        let user_profile = &mut ctx.accounts.recipient_profile;
        user_profile.interaction_count += 1;

        // Validate token mint matches sender and recipient token accounts
        if ctx.accounts.sender_token_account.mint != ctx.accounts.token_mint.key()
            || ctx.accounts.recipient_token_account.mint != ctx.accounts.token_mint.key()
        {
            return err!(ErrorCode::InvalidTokenMint);
        }

        // Transfer tokens
        let cpi_accounts = Transfer {
            from: ctx.accounts.sender_token_account.to_account_info(),
            to: ctx.accounts.recipient_token_account.to_account_info(),
            authority: ctx.accounts.sender.to_account_info(),
        };
        let cpi_program = ctx.accounts.token_program.to_account_info();
        token::transfer(CpiContext::new(cpi_program, cpi_accounts), amount)?;

        // Emit event for frontend
        emit!(TipEvent {
            sender: ctx.accounts.sender.key(),
            recipient: ctx.accounts.recipient.key(),
            token_mint: ctx.accounts.token_mint.key(),
            amount,
            action,
            timestamp: Clock::get()?.unix_timestamp,
        });

        msg!(
            "Tipped {} tokens ({}) for {} to {}",
            amount,
            ctx.accounts.token_mint.key(),
            action,
            ctx.accounts.recipient.key()
        );
        Ok(())
    }

    // Create a paywall for content
    pub fn create_paywall(
        ctx: Context<CreatePaywall>,
        content_id: String,
        price: u64,
        token_mint: Pubkey,
    ) -> Result<()> {
        let paywall = &mut ctx.accounts.paywall;
        paywall.creator = ctx.accounts.creator.key();
        paywall.content_id = content_id.clone();
        paywall.price = price;
        paywall.token_mint = token_mint;
        paywall.access_count = 0;
        msg!(
            "Created paywall for content {} with price {} ({})",
            content_id,
            price,
            token_mint
        );
        Ok(())
    }

    // Unlock paywall by paying with the specified token
    pub fn unlock_paywall(ctx: Context<UnlockPaywall>, content_id: String) -> Result<()> {
        let paywall = &mut ctx.accounts.paywall;
        let amount = paywall.price;

        // Validate token mint matches paywall and token accounts
        if paywall.token_mint != ctx.accounts.token_mint.key()
            || ctx.accounts.user_token_account.mint != ctx.accounts.token_mint.key()
            || ctx.accounts.creator_token_account.mint != ctx.accounts.token_mint.key()
        {
            return err!(ErrorCode::InvalidTokenMint);
        }

        // Transfer tokens to creator
        let cpi_accounts = Transfer {
            from: ctx.accounts.user_token_account.to_account_info(),
            to: ctx.accounts.creator_token_account.to_account_info(),
            authority: ctx.accounts.user.to_account_info(),
        };
        let cpi_program = ctx.accounts.token_program.to_account_info();
        token::transfer(CpiContext::new(cpi_program, cpi_accounts), amount)?;

        // Update paywall access count
        paywall.access_count += 1;

        // Emit event
        emit!(PaywallUnlockEvent {
            user: ctx.accounts.user.key(),
            creator: paywall.creator,
            content_id,
            token_mint: paywall.token_mint,
            amount,
            timestamp: Clock::get()?.unix_timestamp,
        });

        msg!(
            "Unlocked paywall for content {} by {}",
            paywall.content_id,
            ctx.accounts.user.key()
        );
        Ok(())
    }
}

// Account structures
#[derive(Accounts)]
pub struct InitializeUser<'info> {
    #[account(
        init,
        payer = user,
        space = 8 + 32 + 8 + 100, // Discriminator + Pubkey + u64 + padding
        seeds = [b"user_profile", user.key().as_ref()],
        bump
    )]
    pub user_profile: Account<'info, UserProfile>,
    #[account(mut)]
    pub user: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct Tip<'info> {
    #[account(
        mut,
        seeds = [b"user_profile", recipient.key().as_ref()],
        bump
    )]
    pub recipient_profile: Account<'info, UserProfile>,
    #[account(mut)]
    pub sender_token_account: Account<'info, TokenAccount>,
    #[account(mut)]
    pub recipient_token_account: Account<'info, TokenAccount>,
    #[account(mut)]
    pub sender: Signer<'info>,
    pub recipient: AccountInfo<'info>,
    pub token_mint: AccountInfo<'info>, // Token mint for the SPL token
    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
#[instruction(content_id: String)]
pub struct CreatePaywall<'info> {
    #[account(
        init,
        payer = creator,
        space = 8 + 32 + 32 + 8 + 32 + 8 + 100, // Discriminator + Pubkey + String + u64 + Pubkey + u64 + padding
        seeds = [b"paywall", creator.key().as_ref(), content_id.as_bytes()],
        bump
    )]
    pub paywall: Account<'info, Paywall>,
    #[account(mut)]
    pub creator: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(content_id: String)]
pub struct UnlockPaywall<'info> {
    #[account(
        mut,
        seeds = [b"paywall", paywall.creator.as_ref(), content_id.as_bytes()],
        bump
    )]
    pub paywall: Account<'info, Paywall>,
    #[account(mut)]
    pub user_token_account: Account<'info, TokenAccount>,
    #[account(mut)]
    pub creator_token_account: Account<'info, TokenAccount>,
    #[account(mut)]
    pub user: Signer<'info>,
    pub token_mint: AccountInfo<'info>, // Token mint for the SPL token
    pub token_program: Program<'info, Token>,
}

// Data structures
#[account]
pub struct UserProfile {
    pub owner: Pubkey,          // User's public key
    pub interaction_count: u64, // Number of interactions (tips received)
}

#[account]
pub struct Paywall {
    pub creator: Pubkey,      // Creator's public key
    pub content_id: String,   // Unique content identifier
    pub price: u64,          // Price in tokens
    pub token_mint: Pubkey,   // SPL token mint for payments
    pub access_count: u64,    // Number of users who unlocked
}

// Events for frontend integration
#[event]
pub struct TipEvent {
    pub sender: Pubkey,
    pub recipient: Pubkey,
    pub token_mint: Pubkey,
    pub amount: u64,
    pub action: String,
    pub timestamp: i64,
}

#[event]
pub struct PaywallUnlockEvent {
    pub user: Pubkey,
    pub creator: Pubkey,
    pub content_id: String,
    pub token_mint: Pubkey,
    pub amount: u64,
    pub timestamp: i64,
}

// Custom errors
#[error_code]
pub enum ErrorCode {
    #[msg("Invalid token mint provided")]
    InvalidTokenMint,
}
